#!/usr/bin/env bash
# SSR UI live smoke: real server, temp company, curl through every UI flow.
# Exists because unit tests missed a real bug (journal commit rebuilt the
# entry with a fresh as_of, so the digest-bound token always failed live).
# Needs no secrets. Loopback only. Cleans up after itself.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PORT="${UI_SMOKE_PORT:-3196}"
BASE="http://127.0.0.1:${PORT}"
API_PID=""
WORK=""

cleanup() {
  if [[ -n "$API_PID" ]]; then
    kill "$API_PID" 2>/dev/null || true
    wait "$API_PID" 2>/dev/null || true
  fi
  [[ -n "$WORK" && -d "$WORK" ]] && rm -rf "$WORK" || true
}
trap cleanup EXIT

fail() { echo "UI_SMOKE_FAIL: $*" >&2; exit 1; }

# flash <html> -> prints flash line(s)
flash() { rg -o 'flash [a-z]*" role="status">[^<]*' <<<"$1" | head -1 || true; }

expect_ok() { # expect_ok <label> <html>
  local f; f="$(flash "$2")"
  [[ "$f" == *'flash ok'* ]] || fail "$1: expected flash ok, got: ${f:-<none>}"
  echo "ok: $1 · ${f#*status\">}"
}

expect_err_contains() { # expect_err_contains <label> <needle> <html>
  local f; f="$(flash "$3")"
  [[ "$f" == *"$2"* ]] || fail "$1: expected error containing '$2', got: ${f:-<none>}"
  echo "ok: $1 (fail-closed) · ${f#*status\">}"
}

input_value() { # input_value <html> <name> -> unescaped value
  rg -o "name=\"$2\" value=\"[^\"]*" <<<"$1" | head -1 | cut -d'"' -f4 \
    | python3 -c 'import sys,html; print(html.unescape(sys.stdin.read().strip()))'
}

cargo build -q -p klarbog-api -p klarbog-cli

WORK="$(mktemp -d /tmp/klarbog-ui-smoke.XXXXXX)"
COMPANY="$WORK/demo"
./target/debug/klarbog init --company "$COMPANY" --name "Smoke ApS" --actor ui-dev >/dev/null

KLARBOG_ALLOWLIST_ROOT="$WORK" KLARBOG_BIND="127.0.0.1:${PORT}" \
  ./target/debug/klarbog-api >"$WORK/server.log" 2>&1 &
API_PID=$!

for _ in $(seq 1 30); do
  curl -sf "$BASE/health" >/dev/null 2>&1 && break
  kill -0 "$API_PID" 2>/dev/null || { cat "$WORK/server.log" >&2; fail "server died on start"; }
  sleep 0.5
done
curl -sf "$BASE/health" >/dev/null || fail "server not healthy on $BASE"

JAR="$WORK/cookies.txt"
post() { curl -s -c "$JAR" -b "$JAR" -X POST "$BASE$1" "${@:2}"; }
getp() { curl -s -b "$JAR" "$BASE$1"; }

# --- settings + parties (create redirects 303; verify via list) ---
post /ui/settings --data-urlencode "company=$COMPANY" >/dev/null
post /ui/parties --data-urlencode action=create --data-urlencode "display_name=Smoke Kunde" >/dev/null
getp /ui/parties | rg -q "Smoke Kunde" || fail "party missing after create"
echo "ok: party create · listed"
PARTY_ID="$(getp /ui/parties | rg -o 'name="party_id" value="[^"]*' | cut -d'"' -f4 | head -1)"
[[ -n "$PARTY_ID" ]] || fail "no party_id on parties page"

# --- journal: preview -> commit (digest-bound entry_json regression) ---
JFORM=(--data-urlencode "memo=udgift #vat25 #receipt"
  --data-urlencode account1=3000 --data-urlencode direction1=debit --data-urlencode amount1=12500
  --data-urlencode account2=2000 --data-urlencode direction2=credit --data-urlencode amount2=12500
  --data-urlencode moms_gross=12500 --data-urlencode "moms_memo=x")
PAGE="$(post /ui/journal --data-urlencode action=preview "${JFORM[@]}" --data-urlencode confirm_token=)"
expect_ok "journal preview" "$PAGE"
TOKEN="$(input_value "$PAGE" confirm_token)"; EJSON="$(input_value "$PAGE" entry_json)"
[[ -n "$TOKEN" && -n "$EJSON" ]] || fail "journal preview: missing token/entry_json"
expect_ok "journal commit" "$(post /ui/journal --data-urlencode action=commit "${JFORM[@]}" \
  --data-urlencode "confirm_token=$TOKEN" --data-urlencode "entry_json=$EJSON")"

# --- journal: moms_apply books a 3-leg VAT split via preview -> commit ---
PAGE="$(post /ui/journal --data-urlencode action=moms_apply \
  --data-urlencode moms_gross=12500 --data-urlencode "moms_memo=kontor #vat25 #receipt" \
  --data-urlencode account1=3000 --data-urlencode account2=2000)"
expect_ok "moms split preview" "$PAGE"
TOKEN="$(input_value "$PAGE" confirm_token)"; EJSON="$(input_value "$PAGE" entry_json)"
[[ -n "$TOKEN" && -n "$EJSON" ]] || fail "moms split: missing token/entry_json"
rg -q '4000' <<<"$EJSON" || fail "moms split: no Købsmoms 4000 leg in entry_json"
expect_ok "moms split commit" "$(post /ui/journal --data-urlencode action=commit \
  --data-urlencode "confirm_token=$TOKEN" --data-urlencode "entry_json=$EJSON")"
getp /ui/chart | rg -q '4000' || fail "moms split: 4000 missing from chart balances"

# --- invoices: create -> send -> paid preview -> commit ---
expect_ok "invoice create" "$(post /ui/invoices --data-urlencode action=create \
  --data-urlencode "party_id=$PARTY_ID" --data-urlencode kind=sale \
  --data-urlencode description=Smoke --data-urlencode amount_minor=50000)"
INV="$(getp /ui/invoices | rg -o 'name="invoice_id" value="[^"]*' | cut -d'"' -f4 | head -1)"
[[ -n "$INV" ]] || fail "no invoice_id on invoices page"
expect_ok "invoice send" "$(post /ui/invoices --data-urlencode action=send --data-urlencode "invoice_id=$INV")"
PAGE="$(post /ui/invoices --data-urlencode action=paid_preview --data-urlencode "invoice_id=$INV")"
expect_ok "invoice paid preview" "$PAGE"
TOKEN="$(input_value "$PAGE" confirm_token)"; EJSON="$(input_value "$PAGE" entry_json)"
expect_ok "invoice payment commit" "$(post /ui/invoices --data-urlencode action=commit_payment \
  --data-urlencode "confirm_token=$TOKEN" --data-urlencode "entry_json=$EJSON")"

# --- bank: needs a fresh sent invoice (previous is now paid) ---
expect_ok "bank invoice create" "$(post /ui/invoices --data-urlencode action=create \
  --data-urlencode "party_id=$PARTY_ID" --data-urlencode kind=sale \
  --data-urlencode description=Bankmatch --data-urlencode amount_minor=70000)"
INV2="$(getp /ui/invoices | rg -o 'name="invoice_id" value="[^"]*' | cut -d'"' -f4 | head -1)"
expect_ok "bank invoice send" "$(post /ui/invoices --data-urlencode action=send --data-urlencode "invoice_id=$INV2")"

BROW=(--data-urlencode provider=generic_dk --data-urlencode csv=
  --data-urlencode row_date=2026-05-20 --data-urlencode "row_text=Betaling Bankmatch"
  --data-urlencode row_amount=70000)
PAGE="$(post /ui/bank --data-urlencode action=reconcile_suggest "${BROW[@]}")"
expect_ok "bank suggest" "$PAGE"
rg -q "$INV2" <<<"$PAGE" || fail "bank suggest: invoice $INV2 not suggested"
expect_err_contains "bank apply below threshold" "below safe threshold" \
  "$(post /ui/bank --data-urlencode action=apply_preview "${BROW[@]}" \
    --data-urlencode "invoice_id=$INV2" --data-urlencode row_index=0)"
PAGE="$(post /ui/bank --data-urlencode action=apply_preview "${BROW[@]}" \
  --data-urlencode "invoice_id=$INV2" --data-urlencode row_index=0 --data-urlencode force=on)"
expect_ok "bank apply (forced)" "$PAGE"
TOKEN="$(input_value "$PAGE" confirm_token)"; EJSON="$(input_value "$PAGE" entry_json)"
expect_ok "bank commit" "$(post /ui/bank --data-urlencode action=commit_apply \
  --data-urlencode provider=generic_dk --data-urlencode csv= --data-urlencode row_date= \
  --data-urlencode row_text= --data-urlencode row_amount= \
  --data-urlencode "confirm_token=$TOKEN" --data-urlencode "entry_json=$EJSON")"

# --- bilag: upload -> exception close -> gdpr export -> remove ---
echo smoke > "$WORK/kvit.txt"
expect_ok "bilag upload" "$(curl -s -b "$JAR" -X POST "$BASE/ui/bilag/attach" \
  -F kind=receipt -F path_hint=2026/kvit.txt -F "party_id=$PARTY_ID" \
  -F invoice_id= -F notes=smoke -F "file=@$WORK/kvit.txt")"
DOC="$(getp /ui/bilag | rg -o 'name="document_id" value="[^"]*' | cut -d'"' -f4 | head -1)"
expect_ok "exception raise" "$(post /ui/bilag --data-urlencode action=raise_exception \
  --data-urlencode code=smoke --data-urlencode severity=warn --data-urlencode message=t)"
EXC="$(getp /ui/bilag | rg -o 'name="exception_id" value="[^"]*' | cut -d'"' -f4 | head -1)"
expect_ok "exception close" "$(post /ui/bilag --data-urlencode action=close_exception \
  --data-urlencode "exception_id=$EXC")"
expect_ok "gdpr export" "$(post /ui/bilag --data-urlencode action=gdpr_export)"
expect_ok "bilag remove" "$(post /ui/bilag --data-urlencode action=remove_document \
  --data-urlencode "document_id=$DOC" --data-urlencode delete_object=1)"

# --- ledger views reflect the three commits above ---
PAGE="$(getp /ui/journal)"
rg -q "Seneste posteringer" <<<"$PAGE" || fail "journal: postings section missing"
rg -q "udgift #vat25 #receipt" <<<"$PAGE" || fail "journal: committed memo not listed"
echo "ok: journal postings listed"
CHART="$(getp /ui/chart)"
rg -q "Saldi" <<<"$CHART" || fail "chart: balances section missing"
# 3000: 125.00 journal commit + 100.00 net from the moms split = 225.00.
rg -q "225.00 DKK" <<<"$CHART" || fail "chart: expected 225.00 DKK balance on 3000"
# 4000 Købsmoms carries the 25.00 VAT leg from the split.
rg -q "4000" <<<"$CHART" || fail "chart: expected Købsmoms 4000 row"
echo "ok: chart balances listed (incl. moms split)"

# --- momsafregning: settle Købsmoms into 4500 via two-phase ---
PAGE2="$(post /ui/chart --data-urlencode action=settle_preview)"
expect_ok "moms settle preview" "$PAGE2"
TOKEN="$(input_value "$PAGE2" confirm_token)"; EJSON="$(input_value "$PAGE2" entry_json)"
[[ -n "$TOKEN" && -n "$EJSON" ]] || fail "moms settle: missing token/entry_json"
rg -q '4500' <<<"$EJSON" || fail "moms settle: no 4500 leg in entry_json"
expect_ok "moms settle commit" "$(post /ui/chart --data-urlencode action=settle_commit \
  --data-urlencode "confirm_token=$TOKEN" --data-urlencode "entry_json=$EJSON")"
getp /ui/chart | rg -q "Ingen moms at afregne" || fail "moms settle: position not zeroed"
echo "ok: moms position zeroed after settlement"

# --- reversal: reverse the newest posting via its id, commit, net returns to 0 ---
EID="$(input_value "$PAGE" entry_id)"
[[ -n "$EID" ]] || fail "journal: no entry_id for reversal"
PAGE="$(post /ui/journal --data-urlencode action=reverse_preview --data-urlencode "entry_id=$EID")"
expect_ok "reversal preview" "$PAGE"
TOKEN="$(input_value "$PAGE" confirm_token)"; EJSON="$(input_value "$PAGE" entry_json)"
expect_ok "reversal commit" "$(post /ui/journal --data-urlencode action=commit \
  --data-urlencode "confirm_token=$TOKEN" --data-urlencode "entry_json=$EJSON")"
getp /ui/journal | rg -q "tilbageførsel af $EID" || fail "journal: reversal not listed"
echo "ok: reversal listed in postings"

# --- råbalance totals and party saldo ---
CHART="$(getp /ui/chart)"
rg -q "Råbalance" <<<"$CHART" || fail "chart: råbalance totals row missing"
TOTALS="$(rg -o '<th class="money">[0-9.]* DKK</th>' <<<"$CHART" | sort -u | wc -l)"
[[ "$TOTALS" == "1" ]] || fail "chart: total debet != total kredit ($TOTALS distinct totals)"
echo "ok: råbalance totals equal"
PAGE="$(getp /ui/parties)"
rg -q "<th>Saldo</th>" <<<"$PAGE" || fail "parties: saldo column missing"
# Payment entries tag both legs with the party, so a posted party shows a DKK
# saldo (0.00 here: bank debit and AR credit cancel), never the "—" placeholder.
rg -q '<td class="money">[0-9-]+\.[0-9]{2} DKK</td>' <<<"$PAGE" \
  || fail "parties: expected a DKK saldo on the party"
echo "ok: party saldo shown"

echo "UI_SMOKE_OK"
