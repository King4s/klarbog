# Klarbog status

Branch: `rust-dev` (DEV-only).

## Gate

`./scripts/verify.sh` → `VERIFY_OK` (fmt, clippy, tests, linegate, no IEEE floats in crates).

## Surfaces

- CLI: `klarbog`
- API: `127.0.0.1:3195`
- MCP: `klarbog-mcp` (stdio)

Wave 32: OAuth/MCP env-lock hardening
(crate-wide ENV_TEST_LOCK held through await; stripe premature unlock fixed).

See `docs/agent-setup/prompt.md` (product AI prompt) and `swarm/ROADMAP.md`.
