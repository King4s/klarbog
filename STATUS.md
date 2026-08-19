# Klarbog status (2026-08-19)

Branch: `rust-dev`
Path: `/opt/pellucid-software/klarbog` (DEV-only, allowlisted)

## Passed

- `cargo test --workspace` green (types, journal double-entry, sqlite WAL/FK/migrations, core policy, plugin/CRM, API stubs, MCP `tools/list`)
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- line gate (hard 400) and no `f32`/`f64` under `crates/`
- `scripts/swarm_dispatch.py` OK; `swarm/STOP` absent by default

## Crates

klarbog-types, klarbog-journal, klarbog-store-sqlite, klarbog-core,
klarbog-plugin, klarbog-plugin-crm (no journal-write dep), klarbog-api,
klarbog-mcp, klarbog-cli

## Notes

- Upstream TypeScript lives under `reference/typescript/` (git mv).
- API binds `127.0.0.1:3195`. Journal HTTP write is a stub (slice 2).
- Rollback: `git revert` of the slice merge (ADR-005).
