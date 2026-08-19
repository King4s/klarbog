# Changelog

All notable changes to the Klarbog Rust workspace on branch **`rust-dev`**.

Format loosely follows [Keep a Changelog](https://keepachangelog.com/). Version
**0.1.0** (workspace) — pre-release DEV.

## [Unreleased] — rust-dev milestones

### Added — slices 0–9 (core ledger)

- **Slice 0:** Rust workspace scaffold, swarm layout, ADR-001..005 (money, plugins, dev isolation, CRM boundary, swarm).
- **Slice 1:** `i64` minor units, mandatory double-entry journal, actor model, SQLite WAL/FK/migrations.
- **Slice 2:** HTTP API (`klarbog-api` on `127.0.0.1:3195`) and MCP stdio server; two-phase confirm; company path allowlist AuthZ.
- **Slice 3:** Compile-time plugin host; `rules-dk` plugin; applied rules on journal preview/commit.
- **Slice 4:** CRM plugin (`parties.json`) — no journal-write dependency (ADR-004).
- **Slice 5:** Bank CSV import (`GenericDk` semicolon); draft journal suggestions; read-only bank plugin (ADR-006).
- **Slice 6:** Invoice drafts with `party_id`; HTTP invoice endpoints; no direct journal write.
- **Slice 7:** Documents + exceptions plugin; local object storage; HTTP attach/list/exception close.
- **Slice 8:** Agent skills markdown under `docs/skills/`; `klarbog demo` end-to-end smoke.
- **Slice 9:** Retention policy, backup manifest (SHA-256 sidecar), GDPR export stub, expense memo template; CLI + HTTP retention/backup/GDPR.

### Added — payment rails (API-first)

- **Revolut Business API** (ADR-008): live import via `KLARBOG_REVOLUT_API_TOKEN`; CSV offline fallback; `BankProfile::Revolut`.
- **Stripe API** (ADR-009): balance/payout import via `KLARBOG_STRIPE_SECRET_KEY`; CSV offline fallback; `BankProfile::Stripe`.
- HTTP and MCP bank import preview: `source=api|csv`, `provider=revolut|stripe|generic_dk`; fail-closed when API tokens missing.

### Added — Cloudflare R2 (EU)

- `klarbog-storage`: `LocalFsStore` (DEV default) and `R2Store` with SigV4 put/get (ADR-007).
- `KLARBOG_STORAGE=local|r2`; EU jurisdiction fail-closed; optional `KLARBOG_R2_ALLOW_NON_EU=1`.
- Document attach stores bytes via selected backend; HTTP `content_base64` path.

### Added — agent product surface

- End-user AI prompt: [`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md) (MCP/HTTP/CLI bootstrap for any assistant).
- Developer bootstrap split: [`docs/agent-setup/DEVELOPER.md`](docs/agent-setup/DEVELOPER.md).
- Install and run guide: [`docs/INSTALL.md`](docs/INSTALL.md).

### Added — tooling

- `./scripts/verify.sh` — fmt, clippy, tests, line gates, money invariant (no `f32`/`f64` in crates).
- `./scripts/package-dev.sh` — release build + copy binaries to `dist/`.

### Credits

Klarbog is a **Rust port** of
[Rentemester](https://github.com/mikkelkrogsholm/rentemester) by Mikkel Krogsholm
and contributors (MIT). Domain model and agent-friendly design originate there; see
[`NOTICE`](NOTICE) and [`README.md`](README.md).

### Known limitations (DEV)

- Loopback API only; not hardened for internet exposure.
- R2 delete / retention purge enforcement — in progress (roadmap slices 10–13).
- Stripe webhooks and Revolut OAuth — planned hardening (slice 10+).
- No packaged `.deb`/container release yet — build from source.

## Upstream reference

TypeScript implementation and fixtures remain under
[`reference/typescript/`](reference/typescript/) for comparison; the Rust workspace
is new code.
