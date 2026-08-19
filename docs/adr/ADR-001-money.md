# ADR-001: Money representation

## Status
Accepted (Claude CONDITIONAL GO CRITICAL-1)

## Decision
- Amounts are **integer minor units** (`i64`), never `f32`/`f64`.
- Every amount carries an explicit `Currency` (ISO 4217-like code).
- Cross-currency arithmetic is **forbidden** at the type layer.
- Rounding policy: **banker's rounding (half-even)**, documented in one place (`klarbog_types::money`).
- All arithmetic is **checked** (overflow → error), never wrapping.

## Consequences
Ledger math is deterministic and auditable. Property tests must cover balance and rounding edges.
