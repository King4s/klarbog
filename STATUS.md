# Klarbog status

Branch: `rust-dev` (DEV-only).

## Gate

`./scripts/verify.sh` → `VERIFY_OK` (fmt, clippy, tests, linegate, no IEEE floats in crates).

## Surfaces

- CLI: `klarbog`
- API: `127.0.0.1:3195` (default; ADR-014 gated non-loopback)
- MCP: `klarbog-mcp` (stdio)

Wave 37 UI: journal preview/commit + chart list (`wave37_ui_journal_chart`).

Wave 38: `./scripts/live-e2e.sh` — SKIP exit 0 without Revolut/Stripe/R2 keys;
loopback live smoke when set (`wave38_live_e2e_scaffold`).

Wave 39: ADR-014 + `KLARBOG_BIND` / `KLARBOG_ALLOW_NON_LOOPBACK` fail-closed
gate (`wave39_production_bind_gate`) — scaffold only; no public deploy.

Wave 40: ADR-015 TLS edge + `docs/skills/production-checklist.md`
(`wave40_prod_tls_checklist`).

Wave 41: ADR-016 optional `KLARBOG_API_TOKEN` bearer gate
(`wave41_api_token_gate`).

Wave 42: backup/DR skill + offline `backup-smoke.sh`
(`wave42_backup_dr`) — operator-owned; not a managed HA SLA.

See `docs/agent-setup/prompt.md` (product AI prompt) and `swarm/ROADMAP.md`.
