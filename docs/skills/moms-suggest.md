---
name: klarbog-moms-suggest
description: >-
  Optional DK moms 25% inclusive split into net+vat i64 legs for journal draft.
  Preview only — never posts. Use before journal preview when memo has #vat25.
---

# Moms suggest (preview only)

## When to use

- Building journal legs from a **moms-inkl.** gross and the memo will include
  `#vat25` / `moms:25` / `#moms25`.
- Prefer this helper over ad-hoc division (ADR-001 / ADR-011).

## Invariants

- Money is **i64** minor units — never `f32`/`f64`.
- **Never posts.** `auto_post` is always `false`; no ConfirmStore token.
- Without a 25% tag → `{ suggested: false }` (skip; not an error).
- Negative gross or unsupported memo rate → fail closed (HTTP 400 / tool error).
- After suggestion, still run **journal preview → commit**
  ([`journal-preview-commit.md`](journal-preview-commit.md)).

## Core helper

```rust
use klarbog_plugin_rules_dk::moms_post_suggestion;
let Some(s) = moms_post_suggestion(12_500, "office #vat25 #receipt")? else {
    /* no #vat25 tag */
};
// s.net_minor == 10_000, s.vat_minor == 2_500; s.legs = [net, vat]
```

## HTTP (DEV)

`POST /api/v1/journal/moms-suggest`

Headers: `x-klarbog-actor-kind`, `x-klarbog-actor-id` (policy actor).

```json
{ "company": "<company path>", "gross_minor": 12500, "memo": "office #vat25" }
```

Suggested: `suggested: true`, `net_minor`, `vat_minor`, `rate_bps`, `legs`,
`auto_post: false`. No tag: `suggested: false`, `auto_post: false`.

## MCP

`journal_moms_post_suggestion` — same fields + `actor_kind` / `actor_id`.

## Agent checklist

1. Call moms-suggest with gross + memo (include `#vat25` when wanting a split).
2. Copy net/vat into balanced debit/credit legs (chart codes as usual).
3. Preview → commit; respect receipt / account rules on commit.

## See also

- ADR-011: [`docs/adr/ADR-011-moms-suggest.md`](../adr/ADR-011-moms-suggest.md)
- ADR-001 money; skill [`journal-preview-commit.md`](journal-preview-commit.md)
