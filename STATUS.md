# Klarbog status

Branch: `rust-dev` (DEV-only).

## Gate

`./scripts/verify.sh` → `VERIFY_OK` (fmt, clippy, tests, linegate, no IEEE floats in crates).

## Surfaces

- CLI: `klarbog`
- API: `127.0.0.1:3195`
- MCP: `klarbog-mcp` (stdio)

Wave 29: soft-split invoice plugin `lib.rs` → `tests.rs`
(near soft linegate; MCP parity already complete through wave 28).

See `docs/agent-setup/prompt.md` (product AI prompt) and `swarm/ROADMAP.md`.
