---
title: "status"
tags: ["klarbog","rust","dev","slice7"]
project: "Klarbog"
description: ""
updated: "2026-08-20"
---

# Klarbog status (2026-08-20)

Branch `rust-dev` at `/opt/pellucid-software/klarbog`. DEV-only.

## Slice 7 (done)

- `klarbog-plugin-documents`: `documents.json` + `exceptions.json` under company dir
- Document metadata: id, kind, path_hint (relative, no `..`), optional party_id/invoice_id, notes, created_unix_ms
- Exceptions: raise, list open, PATCH close; codes e.g. `missing_attachment`, `unmatched_bank_row`
- HTTP: `POST/GET /api/v1/documents`, `POST/GET/PATCH /api/v1/exceptions` (same AuthZ as CRM)
- `assert_relative_path_hint` in klarbog-core pathguard
- Default registry: Meta + CRM + Invoice + Documents + RulesDk

## Slice 3 (done)

- `klarbog-plugin`: `RulesPlugin` trait, `Registry::list_by_capability`, `validate_rules`
- `klarbog-plugin-rules-dk`: DEV stubs — nonzero legs, memo required, `dk.expense.hint` for 6000 debit
- `journal_preview` / `journal_commit` run rules before token issue / post; `applied_rules` on Envelope
- Default registry: Meta + CRM + RulesDk (`klarbog_core::default_registry`)

## Slice 2 (done)

- `POST /api/v1/journal/preview` + `/commit` (two-phase confirm)
- Actor headers + company path allowlist
- MCP journal tools wired to same core ops

## Passed

- `./scripts/verify.sh` → VERIFY_OK

## Next

Slice 8 — skills + agent-demo (ROADMAP.md).
