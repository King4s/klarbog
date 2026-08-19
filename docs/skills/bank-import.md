---
name: klarbog-bank-import
description: >-
  Import bank transactions into draft journal entries (no posting). GenericDk uses CSV;
  Revolut and Stripe use live API with CSV offline fallback. Use before preview/commit.
---

# Bank import

## When to use

- Turn bank transactions into **draft** `JournalEntry` suggestions.
- Bank plugin is **read-only** (ADR-006) — drafts only; posting via journal-preview-commit.

## Sources and providers

| Provider | Primary source | Fallback |
|----------|----------------|----------|
| `generic_dk` | `csv` (semicolon Danish export) | — |
| `revolut` | `api` (Revolut Business API, ADR-008) | `csv` offline export |
| `stripe` | `api` (Stripe balance transactions, ADR-009) | `csv` offline export |

Dispatch: `import_preview(source, provider, csv?, cfg, actor)`.

Env (never commit/print):

- `KLARBOG_REVOLUT_API_TOKEN`, optional `KLARBOG_REVOLUT_API_BASE`
- `KLARBOG_STRIPE_SECRET_KEY`, optional `KLARBOG_STRIPE_API_BASE`

Missing token when `source=api` → fail closed.

## Amount rules

- All amounts → `MinorAmount` (i64 øre). Half-even rounding from decimal strings.
- Stripe API amounts are already minor units; prefer **net** over gross when fee present.
- Integer-only column values treated as already-minor (CSV).
- Mixed currencies in one batch → reject (fail closed).
- Currency mismatch vs `BankImportConfig.currency` → reject.

## Rust flow

```rust
use klarbog_plugin_bank::{
    import_preview, default_source_for_rail, BankImportConfig, BankImportSource, BankProfile,
};
use klarbog_types::Actor;

let cfg = BankImportConfig::default(); // DKK, accounts 5800/6000/6100
let actor = Actor::agent("bank-import");

// Revolut API (default source for revolut)
let (rows, drafts) = import_preview(
    BankImportSource::Api,
    BankProfile::Revolut,
    None,
    &cfg,
    &actor,
).await?;

// GenericDk CSV
let (rows, drafts) = import_preview(
    BankImportSource::Csv,
    BankProfile::GenericDk,
    Some(csv_text),
    &cfg,
    &actor,
).await?;

for entry in &drafts {
    entry.validate()?; // balanced
}
```

Offline tests: `parse_revolut_api_json` / `parse_stripe_api_json` with fixture JSON (no network).

Fixtures: `tests/fixtures/danish_bank.csv`, `revolut_statement.csv`, `stripe_balance.csv`, `revolut_api.json`, `stripe_api.json`.

## HTTP / MCP

```json
{
  "company": "/path/to/company",
  "source": "api",
  "provider": "revolut",
  "currency": "DKK"
}
```

CSV fallback adds `"source": "csv"` and `"csv": "..."`. Legacy field `profile` aliases `provider`.

## Mapping defaults

- Outflow (negative amount): debit expense `6000`, credit bank `5800`.
- Inflow: debit bank, credit revenue `6100`.
- Memo prefix: `bank:<original text>`.

## Checklist

1. Pick provider + source (`default_source_for_rail` when omitted).
2. Fetch/parse → validate row count and currencies.
3. Map to drafts; skip zero amounts (error).
4. For each draft: journal preview → commit with human/agent confirm.

## Bank reconcile (suggest + apply)

Capability: **Read only**. Never posts journal entries from the bank plugin.

### Suggest

`POST /api/v1/bank/reconcile/suggest` with `company` + `rows` (or CSV/provider). Scores open invoice drafts vs bank lines (`amount_minor` + text tokens). Safe bar: `SAFE_THRESHOLD_BPS` (5000 = 50%). Below that → no suggestion; may raise `unmatched_bank_transaction`.

### Apply → preview suggestion only

`apply_match(company, bank_row, invoice_id, actor, force, row_index)` builds a **payment** `JournalEntry` (invoice kind + `party_id` on legs; memo `bank:{text}:invoice:{id}`). Does **not** post and does **not** mark the invoice paid — host feeds the entry into `/api/v1/journal/preview` then commit (two-phase).

`POST /api/v1/bank/reconcile/apply`:

```json
{
  "company": "/path/to/company",
  "invoice_id": "inv_…",
  "row": { "date": "2026-05-20", "text": "…", "amount_minor": 50000 },
  "force": false
}
```

Or `row_index` + `rows` / CSV provider fields. Response `data.entry` is the journal JSON for preview.

**Unsafe matches** (confidence &lt; `SAFE_THRESHOLD_BPS`): rejected unless `force: true` **and** actor is `user` (agents/system cannot force). When applied, any open `unmatched_bank_transaction` for that bank row related-id is closed.
