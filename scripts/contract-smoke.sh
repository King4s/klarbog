#!/usr/bin/env bash
# Offline HTTP contract smoke (slice 35). No bind, no network.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo test -p klarbog-api contract_smoke
echo "CONTRACT_SMOKE_OK"
