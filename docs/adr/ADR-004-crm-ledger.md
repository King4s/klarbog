# ADR-004: CRM vs ledger + mandatory double-entry

## Status
Accepted (Claude CONDITIONAL GO CRITICAL-2 + CRM)

## Decision
- **Double-entry is mandatory** in `klarbog-journal` and enforced again in the store.
  Reject: unbalanced entries, empty entries, single-leg entries.
- CRM is a separate plugin domain. Ledger may store `PartyId` only — never CRM PII.
- CRM must not call journal post/reverse APIs (no crate dependency on write surface).

## Consequences
Asset/liability facts stay in the ledger; relationships stay in CRM.
