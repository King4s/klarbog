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
- **Stripe webhooks:** `POST /api/v1/webhooks/stripe` (HMAC via `KLARBOG_STRIPE_WEBHOOK_SECRET`); queue drafts only — no auto journal post.
- **Revolut OAuth scaffold:** `GET /api/v1/revolut/oauth/start`, `POST …/callback`; company secrets file mode `0600`.

### Added — Cloudflare R2 (EU)

- `klarbog-storage`: `LocalFsStore` (DEV default) and `R2Store` with SigV4 put/get/delete (ADR-007).
- `KLARBOG_STORAGE=local|r2`; EU jurisdiction fail-closed; optional `KLARBOG_R2_ALLOW_NON_EU=1`.
- Document attach stores bytes via selected backend; HTTP `content_base64` path; `DELETE /api/v1/documents` removes metadata + object.

### Added — bank reconcile + invoice lifecycle

- Reconcile **suggest:** `POST /api/v1/bank/reconcile/suggest` (amount + token scoring; unmatched → exception).
- Invoice statuses `draft|sent|part_paid|paid|void`; `PATCH /api/v1/invoices/status`; `POST /api/v1/invoices/mark-paid` → payment journal **suggestion** (no JournalWrite).
- Retention **purge:** `POST /api/v1/retention/purge` (dry-run default; `confirm:true` to apply).

### Added — agent product surface

- End-user AI prompt: [`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md) (MCP/HTTP/CLI bootstrap for any assistant).
- Developer bootstrap split: [`docs/agent-setup/DEVELOPER.md`](docs/agent-setup/DEVELOPER.md).
- Install and run guide: [`docs/INSTALL.md`](docs/INSTALL.md).

### Added — Wave 2 (harden + wire)

- **Slice 15:** Revolut OAuth **refresh** — `refresh_token` + `expires_at`; `POST /api/v1/revolut/oauth/refresh`; fail-closed without refresh; response never echoes raw tokens.
- **Slice 16:** Stripe webhook **consume** — `POST /api/v1/bank/stripe/consume`; idempotent `queue.consumed`; dry-run / `confirm:true` fail-closed; drafts only.
- **Slice 17:** Bank reconcile **apply** — `POST /api/v1/bank/reconcile/apply` → journal entry **preview suggestion**; `force`+user below safe threshold; no auto post.
- **Slice 18:** GDPR export v1 — company-scoped metadata (`parties`, `invoices`, `documents`, `exceptions`, retention summary); still `gdpr_export.json`, no binary blobs.
- **Slice 19:** `rules-dk` `dk.expense.receipt_required` fail-closed on expense debit without receipt signal; hint when `party_id` present but `#receipt` / `document_id:` missing.
- **Slice 20:** Invoice **part_paid** — `POST /api/v1/invoices/mark-part-paid` (`amount_minor`); MCP `invoice_mark_part_paid_preview`; DEV: amount > 0 and < total; no JournalWrite.
- **Slice 21:** Soft linegate hygiene (split hot modules; soft ≥300 warn, hard >400 fail on non-blank lines).

### Added — Wave 3 (productize)

- **Slice 22:** MCP parity — `bank_stripe_consume`, `bank_reconcile_apply`, `revolut_oauth_refresh` (plus existing invoice mark-paid / mark-part-paid tools).
- **Slice 23:** Invoice **payment ledger** — cumulative payments; remaining = total − sum; mark-paid suggests remaining; overpay rejected.
- **Slice 24:** Stripe consume → reconcile helper — `suggest_from_stripe_consume`; `POST /api/v1/bank/stripe/reconcile-suggest` (still no auto journal post).
- **Slice 25:** GDPR **party erasure** — `erase_party` dry-run/`confirm`; anonymize `display_name`→`erased`; strip doc `party_id` or `--delete-documents`; `journal_refs_retained`; export `note`; `POST /api/v1/gdpr/erase-party`; CLI `klarbog gdpr-erase-party`.
- **Slice 26:** rules-dk VAT **split** — i64 minor + bps (`dk.vat.split_hint`); inclusive 25%/0 helpers.
- **Slice 27:** Reconcile apply optional `preview: true` → ConfirmStore `confirm_token` (wire to journal commit).
- **Slice 29:** CHANGELOG + [`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md) + INSTALL refreshed for wave2/wave3 HTTP+MCP surface.
- Remaining: demo CLI coverage — see [`swarm/ROADMAP.md`](swarm/ROADMAP.md).

### Added — Wave 4 (deepen)

- **Slice 30:** MCP `gdpr_erase_party` (AuthZ + allowlist; `confirm` / `delete_documents`); skill note that journal is immutable (`journal_refs_retained`).
- **Slice 32:** rules-dk **chart stub** — document Klarbog DK codes (`1000` bank, `1500` AR, `4400` AP, `4000`–`6999` expense); optional `dk.bookkeeping.known_account` hint (not hard fail); i64 account parse.
- **Slice 33:** Backup manifest invoice `payments[]` summary (`count`/`total_minor` when present); `erase_party` appends `{company}/gdpr_erase_audit.jsonl` (party_id, unix_ms, mode dry_run|confirm, docs_touched — no secrets).

### Added — Wave 4 (deepen)

- **Slice 31:** Stripe consume → reconcile apply preview — `apply_preview_from_stripe_consume`; `POST /api/v1/bank/stripe/reconcile-apply-preview` (unique safe match → ConfirmStore; still no auto journal commit).
- **Slice 35:** Offline **contract smoke** — `cargo test -p klarbog-api contract_smoke` / `./scripts/contract-smoke.sh` (axum oneshot: health, CRM upsert, invoice draft, mark-part-paid, reconcile suggest, GDPR export, erase-party dry-run, stripe consume dry-run; no bind/network).

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
- Demo CLI may not cover every wave3 surface yet.
- No packaged `.deb`/container release yet — build from source.

## Upstream reference

TypeScript implementation and fixtures remain under
[`reference/typescript/`](reference/typescript/) for comparison; the Rust workspace
is new code.
