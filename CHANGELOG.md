# Changelog

All notable changes to the Klarbog Rust workspace on branch **`rust-dev`**.

Format loosely follows [Keep a Changelog](https://keepachangelog.com/). Version
**0.1.0** (workspace) — pre-release DEV.

## [Unreleased] — rust-dev milestones

### Added — Wave 47 (UI invoice draft create)

- Fakturaer: **Ny kladde** form (party select, sale/purchase, one line
  `amount_minor` i64) → `POST /api/v1/invoices/drafts`; suggestion only, never
  auto-posts.

### Added — Wave 46 (UI bank reconcile-apply preview)

- Bank screen: **Afstem apply (preview)** → `POST /api/v1/bank/reconcile/apply`
  with `preview:true`; fill row from import drafts; ConfirmStore token + optional
  journal commit; never auto-posts.

### Added — Wave 45 (UI invoice payment preview)

- Fakturaer screen: **Sæt sent**, **Delbetalt preview**, **Betalt preview**
  (`preview:true` → ConfirmStore token + `journal_entry`); commit from UI or
  open Journal — never auto-posts (ADR-001 i64; invoice-draft skill).

### Added — Wave 44 (UI moms-suggest soft-split)

- Journal screen: **Moms-forslag** calls `POST /api/v1/journal/moms-suggest`
  (i64 gross + `#vat25`); shows net/vat; optional apply brutto to both legs;
  still preview → commit; never auto-posts (ADR-011).
- Skill `moms-suggest.md` notes the UI panel.

### Added — Wave 43 (session cookie auth residual)

- **ADR-017:** optional HMAC `klarbog_session` when `KLARBOG_API_TOKEN` **and**
  `KLARBOG_SESSION_SECRET` are set — `POST /api/v1/auth/login|logout`;
  middleware accepts Bearer **or** valid cookie; unset secret → Bearer-only.
  Out of scope: OIDC, CSRF cross-site, multi-user.
- Checklist / INSTALL / `.env.example` updated.

### Added — Wave 42 (backup / DR depth)

- Skill: [`docs/skills/backup-restore.md`](docs/skills/backup-restore.md) —
  CLI backup, manifest contents, scratch-allowlist restore expectations;
  explicitly **not** a managed HA / DR SLA.
- Offline `./scripts/backup-smoke.sh` → `BACKUP_SMOKE_OK` (init → backup →
  assert manifest + sidecar).
- Links from production checklist + INSTALL.

### Added — Wave 41 (API bearer gate)

- **ADR-016:** optional `KLARBOG_API_TOKEN` — when set, `/api/v1/*` requires
  Bearer or `x-klarbog-api-token` (constant-time compare); unset = DEV.
  Exempt: `/health`, `/ui/*`, Stripe webhook HMAC path.
- Middleware + tests; UI Indstillinger field; checklist / INSTALL / `.env.example`.

### Added — Wave 40 (TLS edge / production checklist)

- **ADR-015:** TLS at reverse-proxy edge; `klarbog-api` remains plain HTTP;
  prefer loopback upstream; never publish non-loopback without a TLS edge.
- Operator skill: [`docs/skills/production-checklist.md`](docs/skills/production-checklist.md).
- INSTALL + `.env.example` link ADR-015 and the checklist.

### Added — Wave 39 (production bind gate scaffold)

- **ADR-014:** production posture — loopback default (ADR-003); optional
  non-loopback only when `KLARBOG_ALLOW_NON_LOOPBACK=1` **and** explicit
  `KLARBOG_BIND`; residual risks (TLS, auth, backups) documented. No public
  deploy.
- `klarbog-api` bind gate in `main.rs` (fail-closed refuse path + unit tests).
- INSTALL + `.env.example` notes; never enabled by default.

### Added — Wave 38 (live E2E scaffold)

- `./scripts/live-e2e.sh`: fail-closed **SKIP** (exit 0) when Revolut / Stripe /
  R2 secrets are unset; with keys, bounded loopback smoke (`/health`,
  `/api/v1/status`, bank import preview `source=api`) — never prints secrets;
  no production bind.
- Docs: ADR-013 + `docs/skills/live-e2e.md` + INSTALL note.

### Added — Wave 37 (UI journal + chart)

- Journal screen: preview form (memo + two legs, i64 øre via `amount.units`) →
  `confirm_token` → commit against `POST /api/v1/journal/preview|commit`.
- Kontoplan screen: list stub accounts from `GET /api/v1/rules/chart`.

### Added — Wave 36 (local web UI shell)

- **ADR-012:** DEV web UI served by `klarbog-api` at `/ui/` (static `ui/`;
  same origin as REST; loopback only). Optional `KLARBOG_UI_DIR` override.
- First screens: oversigt, parter, fakturaer, indstillinger (actor headers +
  company path; i64 → øre/DKK display helpers).
- Docs: README / INSTALL / `.env.example` for `http://127.0.0.1:3195/ui/` after
  `cargo run -p klarbog-api`.

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
- **Slice 28:** `klarbog demo` covers wave2/3 surfaces offline — CRM + journal, Revolut **CSV** via `import_preview` (no live keys, no Revolut skip TODO), sent→part_paid + remaining ledger, GDPR export, retention get; Revolut **API** without `KLARBOG_REVOLUT_API_TOKEN` fail-closed (`Config`) before any network call.
- **Slice 29:** CHANGELOG + [`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md) + INSTALL refreshed for wave2/wave3 HTTP+MCP surface.

### Added — Wave 4 (deepen)

- **Slice 30:** MCP `gdpr_erase_party` (AuthZ + allowlist; `confirm` / `delete_documents`); skill note that journal is immutable (`journal_refs_retained`).
- **Slice 31:** Stripe consume → reconcile apply preview — `apply_preview_from_stripe_consume`; `POST /api/v1/bank/stripe/reconcile-apply-preview` (unique safe match → ConfirmStore; still no auto journal commit).
- **Slice 32:** rules-dk **chart stub** — document Klarbog DK codes (`1000` bank, `1500` AR, `4400` AP, `4000`–`6999` expense); optional `dk.bookkeeping.known_account` hint (not hard fail); i64 account parse.
- **Slice 33:** Backup manifest invoice `payments[]` summary (`count`/`total_minor` when present); `erase_party` appends `{company}/gdpr_erase_audit.jsonl` (party_id, unix_ms, mode dry_run|confirm, docs_touched — no secrets).
- **Slice 34:** Soft linegate re-check after wave3 growth; split modules at soft ≥300 (bank HTTP tests, rules-dk tests, retention erase HTTP tests).
- **Slice 35:** Offline **contract smoke** — `cargo test -p klarbog-api contract_smoke` / `./scripts/contract-smoke.sh` (axum oneshot: health, CRM upsert, invoice draft, mark-part-paid, reconcile suggest, GDPR export, erase-party dry-run, stripe consume dry-run; no bind/network).

### Added — Wave 5 (polish)

- **Slice 38:** ADR-010 — DK bogføring ~5y retention vs GDPR party erase; journal never deleted by `erase_party`; export/erase document `journal_refs_retained`; invoices/`payments[]` metadata policy. See [`docs/adr/ADR-010-bookkeeping-retention.md`](docs/adr/ADR-010-bookkeeping-retention.md).
- **Slice 40 (light):** Skill cross-link — MCP `bank_reconcile_apply` documents the same `force` + `user` rule as HTTP reconcile apply (agents/system cannot force below safe threshold). See [`docs/skills/bank-import.md`](docs/skills/bank-import.md).
- **Slice 41:** CHANGELOG + [`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md) + INSTALL synced for wave4 endpoints (stripe `reconcile-apply-preview`, MCP erase, chart stub, `contract-smoke.sh`).

### Added — Wave 6 (next gaps)

- **Slice 43:** Optional moms post **suggestion** — `moms_post_suggestion(gross, memo)` via `split_vat25_inclusive` when memo has `#vat25`; `POST /api/v1/journal/moms-suggest` + MCP `journal_moms_post_suggestion`; returns net+vat i64 legs; never auto-posts.
- **Slice 44:** Multi-currency fail-closed regression — Revolut/Stripe CSV + API fixtures reject mixed batch and EUR-only vs company DKK; `import_preview` + HTTP `POST /api/v1/bank/import/preview` return 400 (i64 minor only).
- **Slice 46:** Stripe apply-preview closes prior `unmatched_bank_transaction` on unique safe apply (`unique_safe_apply_closes_prior_unmatched_exception`).
- **Wave 8 / slice 48:** Offline `contract_smoke` covers `POST /api/v1/journal/moms-suggest` (i64 net+vat legs) and multi-currency `import/preview` → **400**.

### Added — Wave 9 (docs)

- **ADR-011 + skill:** moms post suggestion is preview-only, i64 inclusive 25% split, no auto-post. See [`docs/adr/ADR-011-moms-suggest.md`](docs/adr/ADR-011-moms-suggest.md) and [`docs/skills/moms-suggest.md`](docs/skills/moms-suggest.md).

### Added — Wave 10 (fail-closed tests)

- **Slice 50:** Bank/oauth fail-closed regressions — blank `refresh_token`, provider HTTP error preserves company secrets (errors never echo tokens), expired access without refresh fails closed (no network), Stripe API import missing `KLARBOG_STRIPE_SECRET_KEY` → **503** (HTTP + contract-smoke).

### Added — Wave 11 (demo polish)

- **`klarbog demo`:** covers optional moms-suggest (`#vat25` → net 10000 / vat 2500 i64 legs; no tag → none; `auto_post` always false).

### Changed — demo Revolut offline (ROADMAP 28 / `wave33_demo_revolut`)

- **`klarbog demo`:** wave2/3 smoke stays fail-closed and offline-friendly — Revolut via CSV `import_preview` (no live keys, no skip TODO); API without `KLARBOG_REVOLUT_API_TOKEN` returns `Config` before any network call.

### Added — Wave 12 (hygiene)

- **`.gitignore`:** scratch markers `/wave*.md`, `docs/status-*.md`, `/status/` (keeps `STATUS.md` + `docs/status.md`; `slice-*.md` already ignored).

### Added — Wave 13 (packaging docs)

- **`.env.example`:** Klarbog-native `KLARBOG_*` template (allowlist, Revolut, Stripe, R2, package-dev); no upstream product env names or host paths. README / INSTALL / DEVELOPER link the template + `package-dev.sh`.

### Added — Wave 14 (MCP docs sync)

- **Slice 54:** Public MCP allowlist parity — `bank_stripe_reconcile_suggest` + `bank_stripe_reconcile_apply_preview` in [`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md) tools table, [`docs/skills/bank-import.md`](docs/skills/bank-import.md), and INSTALL (same contracts as HTTP; no auto journal post).

### Added — Wave 15 (idle hardening)

- **Slice 55:** HTTP negative-path — `POST /api/v1/journal/moms-suggest` with negative `gross_minor` returns **400** (fail-closed; i64; no suggestion payload).

### Added — Wave 16 (idle hardening)

- **Slice 56:** HTTP negative-path — `POST /api/v1/journal/moms-suggest` with unsupported memo VAT (`vat:12`) returns **400** (fail-closed; no suggestion payload).

### Added — Wave 17 (MCP moms-suggest fail-closed)

- **Slice 57:** MCP negative-path — `journal_moms_post_suggestion` with unsupported memo VAT (`vat:12`) returns envelope **not ok** (fail-closed; i64; no suggestion payload).

### Added — Wave 18 (MCP moms-suggest negative gross)

- **Slice 58:** MCP negative-path — `journal_moms_post_suggestion` with negative `gross_minor` returns envelope **not ok** (fail-closed; i64; no suggestion payload).

### Added — Wave 19 (MCP retention purge)

- **Slice 59:** MCP `retention_purge` mirrors `POST /api/v1/retention/purge` — dry-run unless `confirm: true`; optional `gc_orphan_documents`; closed-exception purge; journal untouched (ADR-010). Prompt / INSTALL / documents-exceptions skill + ADR-010 note.

### Added — Wave 20 (MCP bank reconcile suggest)

- **Slice 60:** MCP `bank_reconcile_suggest` mirrors `POST /api/v1/bank/reconcile/suggest` — `rows` or CSV/provider; `raise_exceptions` default true; AuthZ + allowlist; **no** journal post. Prompt / INSTALL / bank-import skill.

### Added — Wave 21 (MCP GDPR export)

- **Slice 61:** MCP `gdpr_export` mirrors `POST /api/v1/gdpr-export` — company-scoped metadata (`parties`, invoices with i64 `total_minor`, `documents`, `exceptions`, retention + immutable-journal note); writes `gdpr_export.json`; no binary blobs. Prompt / INSTALL / gdpr-erase-party skill.

### Added — Wave 22 (MCP CRM parties)

- **Slice 62:** MCP `crm_upsert_party` + `crm_list_parties` mirror `POST`/`GET /api/v1/crm/parties` — optional `party_id` for get; empty display_name fail-closed; AuthZ deny; **no** journal write (ADR-004). Prompt / INSTALL / crm-parties skill.

### Added — Wave 23 (MCP registry soft-split)

- **Slice 63:** Soft-split `klarbog-mcp` tools registry — `registry.rs` → `registry/{mod,bank,ledger_ops}.rs` (non-blank under soft 300; schemas unchanged).

### Added — Wave 24 (MCP documents + exceptions)

- **Slice 64:** MCP `documents_attach` / `documents_list` / `documents_delete` + `exceptions_raise` / `exceptions_list` / `exceptions_set_open` mirror HTTP `/api/v1/documents` and `/api/v1/exceptions` — AuthZ + allowlist; relative `path_hint`; optional `content_base64`; **no** journal write (ADR-004/007). Prompt / INSTALL / documents-exceptions skill.

### Added — Wave 25 (MCP invoice lifecycle)

- **Slice 65:** MCP `invoice_create_draft` / `invoice_list` / `invoice_patch_status` mirror HTTP `/api/v1/invoices/drafts` + `/api/v1/invoices/status` — i64 `amount_minor`; create returns suggestion-only journal_entry; **no** journal write (ADR-004). Mark-paid previews unchanged. Prompt / INSTALL / invoice-draft skill (`invoice_id` query field aligned).

### Added — Wave 26 (MCP bank reconcile apply preview)

- **Slice 66:** MCP `bank_reconcile_apply` optional `preview: true` mirrors HTTP ConfirmStore path — adds `confirm_token` / `expires_unix_ms` / `payload_digest`; default still suggestion-only; **no** journal commit. Prompt / INSTALL / bank-import skill.

### Added — Wave 27 (MCP Revolut OAuth start/callback)

- **Slice 67:** MCP `revolut_oauth_start` + `revolut_oauth_callback` mirror HTTP `/api/v1/revolut/oauth/start|callback` — AuthZ + allowlist; store under `secrets/revolut.json`; responses never include raw tokens (`revolut_oauth_refresh` already present). Prompt / INSTALL / bank-import skill / ADR-008.

### Added — Wave 28 (MCP status)

- **Slice 68:** MCP `klarbog_status` mirrors `GET /api/v1/status` — `mode`, `allowlist_root`, plugin roster (`id`/`version`/`capabilities`); `bind=stdio` (HTTP uses loopback). Prompt / INSTALL.

### Changed — Wave 29 (soft linegate headroom)

- **Slice 69:** Soft-split `klarbog-plugin-invoice/src/lib.rs` — unit tests moved to `tests.rs` (`#[cfg(test)] mod tests`); invoice types / i64 money / public API unchanged.

### Changed — Wave 30 (soft linegate headroom)

- **Slice 70:** Soft-split `klarbog-plugin-bank` Revolut OAuth unit tests — `oauth/revolut_tests.rs` (298 non-blank) → `revolut_test_support` + `revolut_tests` (start/exchange) + `revolut_refresh_tests` (refresh/resolve); OAuth behavior unchanged.

### Changed — Wave 31 (soft linegate headroom)

- **Slice 71:** Soft-split `klarbog-plugin-retention/src/erase.rs` (290 non-blank) → `erase_tests.rs` via `#[path]`; journal immutable / erase behavior unchanged.

### Fixed — Wave 32 (OAuth/MCP env-lock)

- **Slice 72:** Crate-wide `ENV_TEST_LOCK` held through async Revolut/Stripe/OAuth MCP+API+plugin tests; stripe webhook test no longer drops lock before await (parallel VERIFY flake).

### Added — Wave 32 (invoice mark-paid ConfirmStore preview)

- **Slice 73:** HTTP `POST /api/v1/invoices/mark-paid` + `mark-part-paid` and MCP
  `invoice_mark_paid_preview` / `invoice_mark_part_paid_preview` accept optional
  `preview: true` → ConfirmStore `confirm_token` / `expires_unix_ms` /
  `payload_digest` (same path as journal preview / bank reconcile apply). Default
  remains suggestion-only; **never** auto-commits journal. i64 money. Soft-split
  preview HTTP tests → `invoice_lifecycle_preview_http_tests.rs`. Prompt /
  INSTALL / invoice-draft skill.

### Changed — Wave 33 (soft linegate headroom)

- **Slice 74:** Soft-split `klarbog-api` `bank_http_tests.rs` (293 non-blank) →
  CSV/currency/AuthZ tests + `bank_http_env_tests.rs` (missing-env fail-closed);
  both under soft 300; behavior unchanged.

### Changed — Wave 34 (soft-split verify)

- Verified wave33 bank import HTTP soft-split (`bank_http_tests` + `bank_http_env_tests` via `#[path]` on `bank.rs`); soft <300; coverage unchanged; VERIFY_OK.

### Added — Wave 35 (contract_smoke invoice ConfirmStore preview)

- Offline `contract_smoke`: mark-paid / mark-part-paid `preview: true` asserts
  ConfirmStore `confirm_token` / `expires_unix_ms` / `payload_digest`; default
  path asserts none. Module `contract_smoke/invoice_preview.rs` (soft <300).
  `scripts/contract-smoke.sh` comment updated.

### Changed — Wave 35 (soft linegate headroom)

- **Slice 76:** Soft-split `klarbog-mcp` `invoice_tests.rs` (291 non-blank) →
  CRUD/AuthZ tests + `invoice_preview_tests.rs` (mark-paid ConfirmStore previews);
  both under soft 300; behavior unchanged.

### Added — Wave 35 (rules chart stub surface)

- HTTP `GET /api/v1/rules/chart` and MCP `rules_chart_list` expose rules-dk DEV
  chart stub (codes + labels); AuthZ/allowlist like other read tools; **no**
  journal write. Prompt / INSTALL / journal-preview-commit skill.

### Added — tooling

- `./scripts/verify.sh` — fmt, clippy, tests, line gates, money invariant (no `f32`/`f64` in crates).
- `./scripts/package-dev.sh` — release build + copy binaries to `dist/`; optional sha256 of dist binaries (`PACKAGE_DEV_NO_SHA256=1` to skip); points at `./scripts/contract-smoke.sh`.
- **Slice 45:** INSTALL package section documents contract-smoke + dist checksums (no absolute host paths).

### Credits

Klarbog is a **Rust port** of
[Rentemester](https://github.com/mikkelkrogsholm/rentemester) by Mikkel Krogsholm
and contributors (MIT). Domain model and agent-friendly design originate there; see
[`NOTICE`](NOTICE) and [`README.md`](README.md).

### Known limitations (DEV)

- Loopback API only; not hardened for internet exposure.
- No packaged `.deb`/container release yet — build from source.

## Upstream reference

TypeScript implementation and fixtures remain under
[`reference/typescript/`](reference/typescript/) for comparison; the Rust workspace
is new code.
