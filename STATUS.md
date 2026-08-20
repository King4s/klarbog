# Klarbog status

Branch: `rust-dev` (DEV-only).

## Gate

`./scripts/verify.sh` → `VERIFY_OK` (fmt, clippy, tests, linegate, no IEEE floats in crates).

## Surfaces

- CLI: `klarbog`
- API: `127.0.0.1:3195`
- MCP: `klarbog-mcp` (stdio)

Wave 37 UI: journal preview/commit + chart list (`wave37_ui_journal_chart`).

Wave 38: `./scripts/live-e2e.sh` — SKIP exit 0 without Revolut/Stripe/R2 keys;
loopback live smoke when set (`wave38_live_e2e_scaffold`).

See `docs/agent-setup/prompt.md` (product AI prompt) and `swarm/ROADMAP.md`.
