# Klarbog status

Branch: `rust-dev` (DEV-only).

## Gate

`./scripts/verify.sh` → `VERIFY_OK` (fmt, clippy, tests, linegate, no IEEE floats in crates).

## Surfaces

- CLI: `klarbog`
- API: `127.0.0.1:3195`
- MCP: `klarbog-mcp` (stdio)

Wave 21: MCP `gdpr_export` (HTTP parity; company-scoped metadata → `gdpr_export.json`; i64 totals; no binary blobs).

See `docs/agent-setup/prompt.md` (product AI prompt) and `swarm/ROADMAP.md`.
