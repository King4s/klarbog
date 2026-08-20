#!/usr/bin/env bash
# Offline HTTP contract smoke (slice 35 + 39 + wave8 + wave35). No bind, no network.
# Includes oauth refresh fail-closed (503), mark-paid remaining, moms-suggest,
# multi-currency import preview → 400, and invoice mark-paid / mark-part-paid
# ConfirmStore preview:true (default has no confirm_token).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo test -p klarbog-api contract_smoke
echo "CONTRACT_SMOKE_OK"
