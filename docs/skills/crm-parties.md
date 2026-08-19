---
name: klarbog-crm-parties
description: >-
  CRM party upsert/list/get for Klarbog. Use when linking invoices, documents,
  or journal legs to a counterparty without writing to the ledger.
---

# CRM parties

## When to use

- Create or update a counterparty (customer, vendor) before invoices or documents.
- Attach `party_id` on journal legs for AR/AP traceability.
- **Do not** store CRM PII inside ledger tables — only `PartyId` references (ADR-004).

## Plugin facts

- Crate: `klarbog-plugin-crm`
- Storage: `<company>/parties.json`
- Capabilities: `Read`, `CrmWrite` — **no** `JournalWrite`

## HTTP (DEV)

Headers: `x-klarbog-actor-kind`, `x-klarbog-actor-id` (actor in policy).

### Upsert

`POST /api/v1/crm/parties`

```json
{
  "company": "/path/to/company",
  "display_name": "Nordic Supply ApS",
  "id": null
}
```

Omit `id` to generate; pass existing `id` to update display name.

### List / get

`GET /api/v1/crm/parties?company=<path>` — all parties.

`GET /api/v1/crm/parties?company=<path>&id=<party_id>` — single party.

## Rust (in-process)

```rust
use klarbog_plugin_crm::CrmPlugin;

let crm = CrmPlugin;
let party = crm.upsert(company_path, "Acme København", None)?;
let all = crm.list(company_path)?;
```

## Envelope

Success: `{"ok": true, "data": {"id": "pty_...", "display_name": "..."}}`

Errors: empty display name → `400`; party not found on get → `404`.

## Agent checklist

1. Upsert party before invoice draft or document attach with `party_id`.
2. Persist returned `id` — invoice plugin validates party exists.
3. Never post journal entries from CRM plugin; use journal-preview-commit skill.
