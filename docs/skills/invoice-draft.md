---
name: klarbog-invoice-draft
description: >-
  Create invoice drafts with party_id and optional journal suggestions. No direct
  ledger write — use journal-preview-commit to post suggestions.
---

# Invoice draft

## When to use

- Record a sale or purchase invoice draft before posting to the ledger.
- Generate a balanced journal **suggestion** with `party_id` on all legs.

## Plugin facts

- Crate: `klarbog-plugin-invoice`
- Storage: `<company>/invoices.json`
- Requires existing CRM party (ADR-004, slice 6).
- Capabilities: `Read`, `CrmWrite` — **no** `JournalWrite`

## HTTP (DEV)

`POST /api/v1/invoices/drafts`

```json
{
  "company": "/path/to/company",
  "party_id": "pty_...",
  "kind": "sale",
  "lines": [
    {"description": "Consulting Jan", "amount_minor": 125000, "currency": "DKK"}
  ]
}
```

`kind`: `sale` | `purchase`.

`GET /api/v1/invoices/drafts?company=<path>` — list.

`GET /api/v1/invoices/drafts?company=<path>&invoice_id=<inv_id>` — one invoice.

## MCP

- `invoice_create_draft` — args: `company`, `party_id`, `kind` (`sale`|`purchase`),
  `lines` (`description`, `amount_minor` i64, `currency`), actor. Returns
  `{invoice, journal_entry}` (suggestion only).
- `invoice_list` — args: `company`, optional `invoice_id`, actor.
- `invoice_patch_status` — args: `company`, `invoice_id`,
  `status` (`draft`|`sent`|`part_paid`|`paid`|`void`), actor. **No** journal write.
- `invoice_mark_part_paid_preview` / `invoice_mark_paid_preview` — payment ledger
  previews (still **no** `JournalWrite`; post via journal-preview-commit).

## Journal suggestion (Rust)

```rust
use klarbog_plugin_invoice::{
    InvoicePlugin, InvoiceKind, NewLine, journal_suggestion, InvoiceConfig,
};

let inv = InvoicePlugin.create(company, party_id, InvoiceKind::Sale, vec![
    NewLine { description: "Support".into(), amount_minor: 5000, currency: "DKK".into() },
])?;
let entry = journal_suggestion(&inv, &actor, &InvoiceConfig::default())?;
entry.validate()?;
// Sale: DR 1500 AR / CR 6100 revenue — both legs carry party_id
```

## Lifecycle / payment ledger

Statuses: `draft` | `sent` | `part_paid` | `paid` | `void`.

Payments persist on the invoice as `payments: [{unix_ms, amount_minor, currency}]`.
Remaining = total − sum(payments) (i64 minor units only).

- `POST /api/v1/invoices/mark-part-paid` — amount must be `> 0` and `< remaining`;
  records the payment; overpay rejected.
- `POST /api/v1/invoices/mark-paid` — journal suggestion for **remaining** (not always
  full total); rejects when remaining is 0; records the settling payment.

MCP: `invoice_create_draft`, `invoice_list`, `invoice_patch_status`,
`invoice_mark_part_paid_preview`, `invoice_mark_paid_preview`. Still **no**
`JournalWrite` — post suggestions via journal-preview-commit.

## Validation

- At least one line; positive `amount_minor`; single currency per invoice.
- Non-empty line descriptions.
- Party must exist in `parties.json`.

## VAT split helper (moms 25%)

Invoice line totals are often **gross inclusive**. Before building multi-leg journal suggestions, split with i64 basis points (no floats) via rules-dk:

```rust
use klarbog_plugin_rules_dk::{moms_post_suggestion, split_vat25_inclusive};

let gross = inv.total_minor()?; // treat as moms-inkl.
let s = split_vat25_inclusive(gross)?;
// s.net_minor + s.vat_minor == gross; e.g. 12500 → 10000 + 2500

// Optional preview suggestion when memo will include #vat25 (never auto-posts):
let sug = moms_post_suggestion(gross, "invoice draft #vat25")?;
```

Formula: `vat = gross * 2500 / 12500` (integer); `net = gross - vat` (remainder in net).
Memo tags `#vat25` / `moms:25` on a journal preview apply hint id `dk.vat.split_hint` (non-blocking).

HTTP `POST /api/v1/journal/moms-suggest` and MCP `journal_moms_post_suggestion` return the same net+vat legs for agents. Full contract: [`moms-suggest.md`](moms-suggest.md) / ADR-011.
## Envelope

Create returns invoice + optional embedded suggestion metadata via API.

Errors: `party not found`, `no lines`, `mixed currencies`, overpay / nothing remaining.

## Agent checklist

1. Upsert CRM party first (crm-parties skill).
2. Create draft; note `invoice_id` and total.
3. Call `journal_suggestion` or use API-side suggestion if exposed.
4. Post suggestion through journal-preview-commit — never auto-commit without confirm token.
5. For collections: use mark-part-paid / mark-paid against the payment ledger remaining.
