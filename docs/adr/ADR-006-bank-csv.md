# ADR-006: Bank CSV import (draft entries only)

## Status
Accepted (slice 5 scaffold, DEV)

## Decision
- Bank import lives in `klarbog-plugin-bank` as a **read-only** compile-time plugin (`Capability::Read` only).
- Parse Danish-ish semicolon CSV (`date;text;amount`) with header aliases (`Dato`, `Tekst`, `Beløb`).
- Amounts become integer **øre** via `MinorAmount` — integer columns or decimal strings rounded with **half-even** (`from_ratio_half_even`). No `f64`.
- Each row maps to a **balanced draft** `JournalEntry` (expense/bank or bank/income legs). The plugin never posts; hosts preview/commit separately.
- Default chart: expense `6000`, bank `5800`, income `6100` (overridable via `BankImportConfig`).

## Consequences
Bank reconciliation stays suggest-only until an explicit host commit path runs rules + confirmation.

## HTTP preview (polish 2026-08-20)
- `POST /api/v1/bank/import/preview` — body `{ company, profile: "generic_dk"|"revolut", csv, currency? }`.
- Returns `{ count, drafts: [{ memo, amount_minor }] }`; no ledger post.
- Actor headers + company allowlist (same AuthZ as CRM/invoice routes).

## Backup manifest integrity (polish 2026-08-20)
- After `write_backup_manifest`, `manifest.sha256` sidecar holds hex SHA-256 of final `manifest.json` bytes.
- JSON embeds `content_sha256` (hash of serialized body before that field).
- `verify_manifest_sidecar(path) -> bool` for host/CLI checks.
