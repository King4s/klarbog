#!/usr/bin/env bash
# DEV packaging: release build + copy binaries to dist/
# After packaging: optional sha256 of dist binaries; contract smoke is separate
# (./scripts/contract-smoke.sh — offline HTTP oneshot, no bind/network).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DIST="$ROOT/dist"
mkdir -p "$DIST"

echo "Building workspace (release)..."
cargo build --workspace --release

copy_bin() {
  local name="$1"
  local src="$ROOT/target/release/$name"
  if [[ ! -x "$src" ]]; then
    echo "FAIL: missing binary $src" >&2
    exit 1
  fi
  install -m 755 "$src" "$DIST/$name"
  echo "  -> dist/$name"
}

echo "Copying binaries to dist/"
copy_bin klarbog
copy_bin klarbog-api
copy_bin klarbog-mcp

# Optional integrity note: hex digests of what landed in dist/ (skip with PACKAGE_DEV_NO_SHA256=1)
if [[ "${PACKAGE_DEV_NO_SHA256:-0}" != "1" ]]; then
  echo "sha256 (dist/):"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$DIST/klarbog" "$DIST/klarbog-api" "$DIST/klarbog-mcp"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$DIST/klarbog" "$DIST/klarbog-api" "$DIST/klarbog-mcp"
  else
    echo "  (no sha256sum/shasum; skip)"
  fi
fi

echo "package-dev ok ($(wc -c < "$DIST/klarbog" | tr -d ' ') bytes klarbog)"
echo "next: ./scripts/contract-smoke.sh  # offline HTTP contract smoke"
