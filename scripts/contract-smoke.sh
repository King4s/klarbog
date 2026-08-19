#!/usr/bin/env bash
# Offline HTTP contract smoke (slice 35 + 39). No bind, no network.
# Includes oauth refresh fail-closed (503) + mark-paid remaining after part_paid.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo test -p klarbog-api contract_smoke
echo "CONTRACT_SMOKE_OK"
