#!/usr/bin/env bash
# Live E2E smoke (wave38). Fail-closed without keys: SKIP exit 0.
# With keys: bounded loopback smoke — health, status, bank import preview
# (source=api for Revolut + Stripe). Never prints secret values.
# Loopback only (127.0.0.1:3195). No production bind.
set -euo pipefail
# Never dump env / token values (also if someone runs bash -x).
set +x

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

API_BASE="${KLARBOG_API_BASE:-http://127.0.0.1:3195}"
CURL_MAX="${LIVE_E2E_CURL_MAX_TIME:-30}"
STARTED_API=0
API_PID=""
COMPANY_DIR=""

cleanup() {
  if [[ -n "$COMPANY_DIR" && -d "$COMPANY_DIR" ]]; then
    rm -rf "$COMPANY_DIR" || true
  fi
  if [[ "$STARTED_API" -eq 1 && -n "$API_PID" ]]; then
    kill "$API_PID" 2>/dev/null || true
    wait "$API_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

missing=()
[[ -z "${KLARBOG_REVOLUT_API_TOKEN:-}" ]] && missing+=("KLARBOG_REVOLUT_API_TOKEN")
[[ -z "${KLARBOG_STRIPE_SECRET_KEY:-}" ]] && missing+=("KLARBOG_STRIPE_SECRET_KEY")
for v in \
  KLARBOG_R2_ACCOUNT_ID \
  KLARBOG_R2_ACCESS_KEY_ID \
  KLARBOG_R2_SECRET_ACCESS_KEY \
  KLARBOG_R2_BUCKET
do
  if [[ -z "${!v:-}" ]]; then
    missing+=("$v")
  fi
done

if ((${#missing[@]} > 0)); then
  echo "SKIP: live E2E requires secrets (unset: ${missing[*]})."
  echo "Export KLARBOG_REVOLUT_API_TOKEN, KLARBOG_STRIPE_SECRET_KEY, and"
  echo "KLARBOG_R2_ACCOUNT_ID / ACCESS_KEY_ID / SECRET_ACCESS_KEY / BUCKET"
  echo "(see docs/skills/live-e2e.md). Never commit secrets."
  echo "Offline: ./scripts/contract-smoke.sh  and  ./scripts/verify.sh"
  echo "LIVE_E2E_SKIP"
  exit 0
fi

export KLARBOG_ALLOWLIST_ROOT="${KLARBOG_ALLOWLIST_ROOT:-$ROOT}"
export KLARBOG_STORAGE="${KLARBOG_STORAGE:-r2}"

echo "live-e2e: keys present (values not printed); loopback smoke starting"

BIN=""
if [[ -x "$ROOT/target/release/klarbog-api" ]]; then
  BIN="$ROOT/target/release/klarbog-api"
elif [[ -x "$ROOT/target/debug/klarbog-api" ]]; then
  BIN="$ROOT/target/debug/klarbog-api"
else
  echo "building klarbog-api (debug)…"
  cargo build -p klarbog-api -q
  BIN="$ROOT/target/debug/klarbog-api"
fi

CLI_BIN=""
if [[ -x "$ROOT/target/release/klarbog" ]]; then
  CLI_BIN="$ROOT/target/release/klarbog"
elif [[ -x "$ROOT/target/debug/klarbog" ]]; then
  CLI_BIN="$ROOT/target/debug/klarbog"
else
  cargo build -p klarbog-cli -q
  CLI_BIN="$ROOT/target/debug/klarbog"
fi

if ! curl -sS --max-time 2 "$API_BASE/health" >/dev/null 2>&1; then
  echo "live-e2e: starting klarbog-api on 127.0.0.1:3195"
  "$BIN" >/tmp/klarbog-live-e2e-api.log 2>&1 &
  API_PID=$!
  STARTED_API=1
  for _ in $(seq 1 40); do
    if curl -sS --max-time 1 "$API_BASE/health" >/dev/null 2>&1; then
      break
    fi
    if ! kill -0 "$API_PID" 2>/dev/null; then
      echo "FAIL: klarbog-api exited before healthy (see /tmp/klarbog-live-e2e-api.log)" >&2
      exit 1
    fi
    sleep 0.25
  done
fi

health_body="$(curl -sS --max-time "$CURL_MAX" "$API_BASE/health")"
echo "$health_body" | grep -q '"ok":true' || {
  echo "FAIL: /health not ok" >&2
  exit 1
}
echo "live-e2e: health OK"

status_code="$(curl -sS -o /tmp/klarbog-live-e2e-status.json -w '%{http_code}' \
  --max-time "$CURL_MAX" "$API_BASE/api/v1/status")"
[[ "$status_code" == "200" ]] || {
  echo "FAIL: /api/v1/status HTTP $status_code" >&2
  exit 1
}
# Do not dump full status JSON (allowlist paths). Check mode only.
grep -q '"mode"' /tmp/klarbog-live-e2e-status.json || {
  echo "FAIL: status body missing mode" >&2
  exit 1
}
echo "live-e2e: status OK"

COMPANY_DIR="$KLARBOG_ALLOWLIST_ROOT/companies/live-e2e-$$"
mkdir -p "$(dirname "$COMPANY_DIR")"
"$CLI_BIN" init --company "$COMPANY_DIR" --name "Live E2E ApS" --actor owner >/dev/null

actor_hdr=(-H "x-klarbog-actor-kind: user" -H "x-klarbog-actor-id: owner" \
  -H "content-type: application/json")

import_preview() {
  local provider="$1"
  local out="/tmp/klarbog-live-e2e-import-${provider}.json"
  local code
  code="$(curl -sS -o "$out" -w '%{http_code}' --max-time "$CURL_MAX" \
    "${actor_hdr[@]}" \
    -d "{\"company\":\"$COMPANY_DIR\",\"provider\":\"$provider\",\"source\":\"api\",\"currency\":\"DKK\"}" \
    "$API_BASE/api/v1/bank/import/preview")"
  # Accept 200 (drafts) or upstream 5xx mapped as gateway — surface non-config failures.
  if [[ "$code" != "200" ]]; then
    # Print status + error strings only (API never echoes tokens).
    echo "FAIL: import preview provider=$provider HTTP $code" >&2
    python3 - "$out" <<'PY' >&2 || true
import json, sys
p = sys.argv[1]
try:
    d = json.load(open(p, encoding="utf-8"))
except Exception as e:
    print(f"(unreadable body: {e})")
    raise SystemExit(0)
errs = d.get("errors") or []
for e in errs[:5]:
    print(f"  error: {e}")
PY
    exit 1
  fi
  local count
  count="$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1],encoding="utf-8")); print((d.get("data") or {}).get("count", "?"))' "$out")"
  echo "live-e2e: import preview provider=$provider OK (count=$count)"
}

import_preview revolut
import_preview stripe

echo "LIVE_E2E_OK"
