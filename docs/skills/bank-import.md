---
name: klarbog-bank-import
description: >-
  Parse bank CSV into draft journal entries (no posting). Covers GenericDk and
  Revolut profiles. Use before preview/commit for bank reconciliation.
---

# Bank CSV import

## When to use

- Turn a bank export CSV into **draft** `JournalEntry` suggestions.
- Bank plugin is **read-only** (ADR-006) — drafts only; posting via journal-preview-commit.

## Profiles

| Profile | Format | Parser |
|---------|--------|--------|
| `GenericDk` | Semicolon, Danish amounts | `parse_bank_csv` |
| `Revolut` | Comma, quoted RFC-ish (ADR-008) | `parse_revolut_csv` |

Dispatch: `parse_bank_csv_with_profile(BankProfile::Revolut, csv, currency)`.

Revolut columns: `Completed Date`, `Description`, `Amount`, `Currency`.

## Amount rules

- All amounts → `MinorAmount` (i64 øre). Half-even rounding from decimal strings.
- Integer-only column values treated as already-minor.
- Mixed currencies in one batch → reject (fail closed).
- Currency mismatch vs `BankImportConfig.currency` → reject.

## Rust flow

```rust
use klarbog_plugin_bank::{
    parse_bank_csv, parse_revolut_csv, draft_entries_from_rows,
    BankImportConfig, BankProfile,
};
use klarbog_types::Actor;

let cfg = BankImportConfig::default(); // DKK, accounts 5800/6000/6100
let rows = parse_bank_csv(csv_text)?;
// Revolut: parse_revolut_csv(csv, Some(&cfg.currency))?

let actor = Actor::agent("bank-import");
let drafts = draft_entries_from_rows(&rows, &cfg, &actor)?;
for entry in &drafts {
    entry.validate()?; // balanced
}
// Each draft: preview → commit per entry
```

Fixtures: `crates/klarbog-plugin-bank/tests/fixtures/danish_bank.csv`, `revolut_statement.csv`.

## Mapping defaults

- Outflow (negative amount): debit expense `6000`, credit bank `5800`.
- Inflow: debit bank, credit revenue `6100`.
- Memo prefix: `bank:<original text>`.

## Agent demo

`klarbog demo` prints `bank_draft_count_generic_dk` and `bank_draft_count_revolut` from embedded fixtures (no network).

## TODO (future)

- Revolut Business OAuth API — out of scope until separate ADR; CSV-first today.

## Checklist

1. Detect profile from CSV header (semicolon vs Revolut columns).
2. Parse → validate row count and currencies.
3. Map to drafts; skip zero amounts (error).
4. For each draft: journal preview → commit with human/agent confirm.
