# Klarbog status

Branch: `rust-dev` (DEV-only).

## Gate

`./scripts/verify.sh` → `VERIFY_OK` (fmt, clippy, tests, linegate, no IEEE floats in crates).

## Surfaces

- CLI: `klarbog`
- API: `127.0.0.1:3195`
- MCP: `klarbog-mcp` (stdio)

Wave 33 demo Revolut (`wave33_demo_revolut` / ROADMAP 28): `klarbog demo`
offline CSV + API fail-closed without live keys.

See `docs/agent-setup/prompt.md` (product AI prompt) and `swarm/ROADMAP.md`.
