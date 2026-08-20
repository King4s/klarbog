# ADR-011: Optional moms post suggestion (preview only)

## Status
Accepted (wave 9 — documents slice 43)

## Context
Danish expense and invoice drafts often arrive as **moms-inkl.** (tax-inclusive)
gross amounts. Agents need a deterministic way to turn a 25% gross into net +
VAT legs before calling journal preview/commit.

Rules-dk already emits non-blocking `dk.vat.split_hint` when a memo carries
`#vat25` / `moms:25` / `#moms25`. That hint does **not** invent leg amounts.
Without a shared helper, agents re-implement inclusive split math — risking
`f32`/`f64` or inconsistent remainder handling (ADR-001).

## Decision

### Preview-only suggestion surface
- Expose `moms_post_suggestion(gross_minor, memo)` in `klarbog-plugin-rules-dk`.
- HTTP: `POST /api/v1/journal/moms-suggest` (AuthZ + company allowlist).
- MCP: `journal_moms_post_suggestion`.
- Response always includes `auto_post: false`. The helper **never** posts, never
  creates a ConfirmStore token, and never mutates the ledger.

### When a suggestion is returned
- Memo must request standard 25% via `#vat25`, `moms:25`, or `#moms25`
  (same parse path as rules-dk VAT rate tags).
- No 25% tag → success with `suggested: false` (optional helper, not an error).
- `#vat0` / `moms:0` → no suggestion (rate tag only; zero VAT is trivial).
- Unsupported memo rates (e.g. `vat:12`) → **400** / error (fail closed).
- Negative `gross_minor` → error.

### Money math (ADR-001)
- `gross_minor`, `net_minor`, `vat_minor` are **i64** minor units only.
- Split uses `split_vat25_inclusive`: `vat = gross * 2500 / 12500` (trunc toward
  zero); `net = gross - vat` so `net + vat == gross` when arithmetic succeeds.
- No `f32`/`f64` anywhere on this path.

### Agent contract
- Suggestion legs are **inputs** the agent copies into a balanced
  `JournalEntry` for normal preview → commit.
- Rules-dk on preview/commit remains authoritative for blocking checks
  (receipt, account range, balance). Moms-suggest does not bypass them.

## Consequences
- Agents have one canonical inclusive 25% split for draft legs.
- Extending to other rates or auto-posting requires a new ADR; this surface
  stays suggestion-only.
- Skill: [`docs/skills/moms-suggest.md`](../skills/moms-suggest.md).
