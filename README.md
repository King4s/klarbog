# Klarbog

**DEV-only** Danish agent-first ledger (Rust rewrite), forked from
[Rentemester](https://github.com/mikkelkrogsholm/rentemester) (MIT — see `LICENSE` + `NOTICE`).

Not production. Not Finance Tracker. Not SeaAid billing.

## Surfaces

- `klarbog` CLI — init / smoke-post
- `klarbog-api` — `127.0.0.1:3195/health`
- `klarbog-mcp` — stdio MCP stub (two-phase confirm tools reserved)

## Quick start

```bash
cargo build --workspace
cargo test --workspace
./scripts/verify.sh
cargo run -p klarbog-cli -- init --company /tmp/klarbog-demo --name "Demo ApS"
cargo run -p klarbog-cli -- smoke-post --company /tmp/klarbog-demo
```

## Swarm

See `swarm/ROADMAP.md` and `docs/adr/`. Create `swarm/STOP` to halt autonomous dispatch.

## Upstream reference

TypeScript Rentemester sources live under `reference/typescript/` for domain fixtures only.
