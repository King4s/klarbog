# Klarbog

**DEV-only** Danish agent-first ledger (Rust rewrite), forked from
[Rentemester](https://github.com/mikkelkrogsholm/rentemester) (MIT — see `LICENSE` + `NOTICE`).

Not production. Not Finance Tracker. Not SeaAid billing.

Workspace crates: `klarbog-types`, `klarbog-journal`, `klarbog-store-sqlite`,
`klarbog-core`, `klarbog-plugin`, `klarbog-plugin-crm`, `klarbog-api`,
`klarbog-mcp`, `klarbog-cli`.
Money is `i64` minor units (never `f64`). Journal double-entry is mandatory.

## Surfaces

- `klarbog` CLI — init / health / smoke-post
- `klarbog-api` — `127.0.0.1:3195/health` plus stub `/api/v1/*`
- `klarbog-mcp` — stdio JSON-RPC stub (`tools/list`, two-phase confirm reserved)

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
