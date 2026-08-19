#!/usr/bin/env bash
# Klarbog verifier gate (Claude CRITICAL + plan)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ALLOW="${ALLOWLIST_ROOT:-$ROOT}"
cd "$ROOT"

if [[ "$ROOT" != "$ALLOW" ]]; then
  echo "FAIL: cwd $ROOT != allowlist $ALLOW" >&2
  exit 10
fi

if [[ -f swarm/STOP ]]; then
  echo "FAIL: swarm/STOP present" >&2
  exit 11
fi

# Path allowlist: dirty files must stay under ROOT
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    case "$f" in
      *)
        abs="$ROOT/$f"
        if [[ "$abs" != "$ALLOW"* ]]; then
          echo "FAIL: path outside allowlist: $f" >&2
          exit 12
        fi
        ;;
    esac
  done < <(git status --porcelain | awk '{print $2}')
fi

# Line gate: soft 300 warn, hard 400 fail (non-blank, rust/py/md sources)
python3 - <<'PY'
from pathlib import Path
root = Path(".")
hard, soft = 400, 300
failed = []
warned = []
for p in root.rglob("*"):
    if p.suffix not in {".rs", ".py", ".md"}:
        continue
    if any(x in p.parts for x in ("target", "reference", ".git", "node_modules")):
        continue
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        print(f"WARN skip unreadable {p}: {e}")
        continue
    lines = [ln for ln in text.splitlines() if ln.strip()]
    n = len(lines)
    if n > hard:
        failed.append((n, str(p)))
    elif n >= soft:
        warned.append((n, str(p)))
for n, p in warned:
    print(f"WARN soft linegate {n}: {p}")
if failed:
    for n, p in failed:
        print(f"FAIL hard linegate {n}: {p}")
    raise SystemExit(13)
print("linegate ok")
PY

# Money invariant: no IEEE floats in Rust crates (ignore comments/docs)
if grep -R -n --include='*.rs' -E '\bf(32|64)\b' crates \
  | grep -vE ':[0-9]+:\s*(//!|///|//)' >/dev/null; then
  echo "FAIL: f32/f64 found under crates/" >&2
  grep -R -n --include='*.rs' -E '\bf(32|64)\b' crates \
    | grep -vE ':[0-9]+:\s*(//!|///|//)' >&2 || true
  exit 14
fi

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
echo "VERIFY_OK"
