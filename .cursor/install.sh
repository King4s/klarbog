#!/usr/bin/env bash
# Cloud Agent install for Rentemester (Bun/TypeScript).
#
# Idempotent repository bootstrap: pins Bun to the version CI uses
# (oven-sh/setup-bun @ 1.3.14) and installs backend + cockpit dependencies
# from the committed lockfiles. Safe to re-run.
set -euo pipefail

BUN_VERSION="1.3.14"
export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
export PATH="$BUN_INSTALL/bin:$PATH"

if [ "$("$BUN_INSTALL/bin/bun" --version 2>/dev/null || true)" != "$BUN_VERSION" ]; then
  echo "Installing Bun v${BUN_VERSION}..."
  curl -fsSL https://bun.sh/install | bash -s "bun-v${BUN_VERSION}"
fi

bun --version

echo "Installing backend dependencies (root)..."
bun install --frozen-lockfile

echo "Installing cockpit dependencies (app/)..."
(cd app && bun install --frozen-lockfile)

echo "Building cockpit SPA so 'rentemester serve' can host it..."
(cd app && bun run build)

echo "Install complete."
