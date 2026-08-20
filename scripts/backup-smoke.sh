#!/usr/bin/env bash
# Offline backup smoke (wave42). Init company → backup → assert manifest.
# Fail-closed: no network; cleans scratch allowlist on exit.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

SCRATCH=""
cleanup() {
  if [[ -n "${SCRATCH}" && -d "${SCRATCH}" ]]; then
    rm -rf "${SCRATCH}" || true
  fi
}
trap cleanup EXIT

CLI_BIN=""
if [[ -x "$ROOT/target/release/klarbog" ]]; then
  CLI_BIN="$ROOT/target/release/klarbog"
elif [[ -x "$ROOT/target/debug/klarbog" ]]; then
  CLI_BIN="$ROOT/target/debug/klarbog"
else
  echo "building klarbog-cli (debug)…"
  cargo build -p klarbog-cli -q
  CLI_BIN="$ROOT/target/debug/klarbog"
fi

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/klarbog-backup-smoke.XXXXXX")"
export KLARBOG_ALLOWLIST_ROOT="$SCRATCH"
COMPANY="$SCRATCH/companies/backup-smoke-aps"

"$CLI_BIN" init --company "$COMPANY" --name "Backup Smoke ApS" --actor owner >/dev/null
out="$("$CLI_BIN" backup --company "$COMPANY")"

manifest=""
if command -v python3 >/dev/null 2>&1; then
  key="$(printf '%s' "$out" | python3 -c '
import json, sys
env = json.load(sys.stdin)
data = env.get("data") or {}
print(data.get("backup_key") or "")
' 2>/dev/null || true)"
  if [[ -n "$key" ]]; then
    manifest="$COMPANY/$key"
  fi
fi

if [[ -z "$manifest" || ! -f "$manifest" ]]; then
  # Narrow glob under the scratch company only
  shopt -s nullglob
  manifests=("$COMPANY"/backups/*/manifest.json)
  shopt -u nullglob
  if ((${#manifests[@]} > 0)); then
    manifest="${manifests[-1]}"
  fi
fi

if [[ -z "$manifest" || ! -f "$manifest" ]]; then
  echo "FAIL: backup manifest missing under $COMPANY/backups" >&2
  exit 1
fi

sidecar="$(dirname "$manifest")/manifest.sha256"
if [[ ! -f "$sidecar" ]]; then
  echo "FAIL: sidecar missing: $sidecar" >&2
  exit 1
fi

hex="$(tr -d '[:space:]' < "$sidecar")"
if [[ ${#hex} -ne 64 ]]; then
  echo "FAIL: sidecar not 64 hex chars" >&2
  exit 1
fi

echo "backup-smoke: manifest ok ($manifest)"
echo "BACKUP_SMOKE_OK"
