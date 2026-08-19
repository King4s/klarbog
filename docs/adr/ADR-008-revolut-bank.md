# ADR-008: Revolut bank import profile

## Status
Accepted (owner 2026-08-19; spelling: **Revolut**, not “ReVolt”)

## Decision
Extend `klarbog-plugin-bank` with an explicit **`BankProfile::Revolut`** CSV/statement parser alongside the existing Danish generic profile.

- Revolut export columns (common): `Completed Date` / `Date`, `Description` / `Reference`, `Amount`, optional `Currency`, `Type`, `State`.
- Delimiter: comma (RFC-ish) with quoted fields; amounts still → `MinorAmount` (i64 øre / half-even). No `f64`.
- Currency mismatch vs company default → reject row or require matching currency (fail closed on mixed currency in one import batch unless all rows share one ISO code).
- Profile remains **Capability::Read** — drafts only; posting via host preview/commit.
- Future: Revolut Business API (OAuth) is out of scope until a separate ADR; CSV-first.

## Consequences
`parse_bank_csv` gains a profile parameter (or `parse_revolut_csv`). Fixtures live under `klarbog-plugin-bank/tests/fixtures/`.
