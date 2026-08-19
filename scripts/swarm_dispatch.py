#!/usr/bin/env python3
"""Minimal Klarbog swarm dispatcher — capacity + STOP + path allowlist."""
from __future__ import annotations

import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    yaml = None  # type: ignore

ROOT = Path(__file__).resolve().parents[1]
SWARM = ROOT / "swarm"
STOP = SWARM / "STOP"
CAPACITY = SWARM / "capacity.yaml"


def load_capacity() -> dict:
    if yaml is None:
        return {
            "max_parallel_worktrees": 3,
            "max_in_flight": 6,
            "failure_limit": 2,
            "allowlist_root": str(ROOT),
            "disk_mb": 8192,
            "max_slices_per_night": 4,
        }
    return yaml.safe_load(CAPACITY.read_text()) or {}


def main() -> int:
    if STOP.exists():
        print("dispatch halted: swarm/STOP present", file=sys.stderr)
        return 2
    cap = load_capacity()
    allow = Path(cap.get("allowlist_root", str(ROOT))).resolve()
    here = ROOT.resolve()
    if here != allow and allow not in here.parents and here != allow:
        # must be inside allowlist root
        try:
            here.relative_to(allow)
        except ValueError:
            print(f"refuse: repo {here} outside allowlist {allow}", file=sys.stderr)
            return 3
    print(
        "ok",
        f"parallel={cap.get('max_parallel_worktrees')}",
        f"in_flight={cap.get('max_in_flight')}",
        f"disk_mb={cap.get('disk_mb')}",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
