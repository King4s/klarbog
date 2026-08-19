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

`GET /api/v1/invoices/drafts?company=<path>&id=<inv_id>` — one draft.

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

## Validation

- At least one line; positive `amount_minor`; single currency per invoice.
- Non-empty line descriptions.
- Party must exist in `parties.json`.

## Envelope

Create returns invoice + optional embedded suggestion metadata via API.

Errors: `party not found`, `no lines`, `mixed currencies`.

## Agent checklist

1. Upsert CRM party first (crm-parties skill).
2. Create draft; note `invoice_id` and total.
3. Call `journal_suggestion` or use API-side suggestion if exposed.
4. Post suggestion through journal-preview-commit — never auto-commit without confirm token.
