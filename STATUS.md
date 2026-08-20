# Klarbog status

Branch: `rust-dev` (DEV-only).

## Gate

`./scripts/verify.sh` → `VERIFY_OK` (fmt, clippy, tests, linegate, no IEEE floats in crates).

## Surfaces

- CLI: `klarbog`
- API: `127.0.0.1:3195`
- MCP: `klarbog-mcp` (stdio)

Wave 25: MCP invoice lifecycle parity (`invoice_create_draft` / `invoice_list` /
`invoice_patch_status`; mark-paid previews already present; no journal write).

See `docs/agent-setup/prompt.md` (product AI prompt) and `swarm/ROADMAP.md`.
