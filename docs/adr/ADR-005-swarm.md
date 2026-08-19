# ADR-005: Klarbog Swarm

## Status
Accepted

## Decision
- In-repo swarm under `swarm/` with `capacity.yaml`, `queue.jsonl`, canonical `ROADMAP.md`.
- Caps: `max_parallel_worktrees=3`, `max_in_flight=6`, `failure_limit=2`, disk + night slice limits.
- Kill switch: presence of `swarm/STOP` halts dispatch.
- Rollback: `git revert` of the slice merge commit on `rust-dev`.

## Consequences
Bounded autonomy for overnight slices without an external task board.
