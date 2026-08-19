#!/usr/bin/env bash
# DEV packaging: release build + copy binaries to dist/
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

echo "package-dev ok ($(wc -c < "$DIST/klarbog" | tr -d ' ') bytes klarbog)"
