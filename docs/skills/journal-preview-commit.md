---
name: klarbog-journal-preview-commit
description: >-
  Two-phase journal posting for Klarbog — preview (token) then commit. Use when
  an agent must post balanced double-entry to the ledger via API, MCP, or core.
---

# Journal preview + commit

## When to use

- Agent needs to **post** a balanced `JournalEntry` to a company ledger.
- Never call `Company::post` directly from untrusted agent flows — use preview/commit.
- Read-only balance checks can use store queries; writes always go through two-phase confirm.

## Invariants

- **Double-entry mandatory**: every entry has ≥2 legs; debits == credits per currency.
- **Money is i64 minor units** (`MinorAmount`); never use `f32`/`f64`.
- **Actor binding**: host overwrites `entry.actor` from authenticated actor — do not spoof in body.
- **Rules-dk** runs on preview and commit; check `applied_rules` (`dk.vat.rate`, `dk.vat.split_hint`, `dk.expense.receipt_hint`, …).
- **Expense receipt (fail-closed):** debit on accounts `4000`–`6999` without `party_id` **and** without a memo receipt signal → `dk.expense.receipt_required` **error** (blocks preview/commit). Soft hint `dk.expense.receipt_hint` only when `party_id` is set but receipt signal is missing.
- **Receipt signals** (whitespace tokens in `memo`, case-insensitive): `#receipt` / `#receipt:…`, or `document_id:<id>` / `document_id=<id>`.
- **VAT split (hint only):** memo `#vat25` / `moms:25` / `#moms25` applies `dk.vat.rate` + `dk.vat.split_hint` (does **not** block). Split amounts with i64 bps only — never f32/f64:

```rust
use klarbog_plugin_rules_dk::split_vat25_inclusive;
// Document convention: amount_minor is gross (moms-inkl.)
let s = split_vat25_inclusive(12_500)?; // net 10_000, vat 2_500
// vat = gross * 2500 / 12500 (integer); net = gross - vat; remainder stays in net
```

Zero-rated `#vat0` / `moms:0` → `dk.vat.rate` only (no split hint).

## HTTP (DEV, loopback)

Headers on every mutating call:

```
x-klarbog-actor-kind: user|agent|system
x-klarbog-actor-id: <id>
```

Actor must appear in company `policy.json`.

### Phase 1 — preview

`POST /api/v1/journal/preview`

```json
{
  "company": "/var/lib/klarbog/companies/demo",
  "entry": {
    "as_of": "2026-01-15T12:00:00Z",
    "memo": "office supplies #receipt",
    "legs": [
      {"account": "6000", "direction": "debit", "amount_minor": 12500, "currency": "DKK"},
      {"account": "5800", "direction": "credit", "amount_minor": 12500, "currency": "DKK"}
    ]
  }
}
```

Response Envelope `data`: `confirm_token`, `payload_digest`, plus top-level `applied_rules`.

### Phase 2 — commit

`POST /api/v1/journal/commit` — same `company` + `entry`, add `confirm_token`.

Token is single-use and bound to `payload_digest` (company path + canonical entry JSON).

## MCP tools

- `journal_post_preview` — same fields as HTTP preview (`actor_kind`, `actor_id`, `company`, `entry`).
- `journal_post_commit` — adds `confirm_token`.

## CLI smoke (no confirm)

`klarbog smoke-post --company <path> --minor 100` posts directly for DEV smoke only.

## Envelope shape

```json
{"ok": true, "data": {...}, "errors": [], "applied_rules": ["rules-dk:..."]}
```

On failure: `ok: false`, `errors: ["..."]`.

## Failure modes

| Error | Meaning |
|-------|---------|
| `actor not in policy` | Add actor to `policy.json` or use allowed id |
| `confirm token required` | Missing/wrong/expired token on commit |
| `rules validation failed` | Entry failed rules-dk (incl. `dk.expense.receipt_required`) |
| `unbalanced journal entry` | Fix leg amounts/directions |

## Agent checklist

1. Build balanced entry (accounts from chart; party_id optional on legs).
2. Preview → store `confirm_token` + verify `applied_rules`.
3. Commit with identical entry payload.
4. On success, `data` contains posted `id` and `digest`.
