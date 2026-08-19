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
