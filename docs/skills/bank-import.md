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

MCP: `bank_reconcile_suggest` — same args (`company`, `rows?` or `provider`/`source`/`csv`, `raise_exceptions?` default true, actor). AuthZ + allowlist. Still **no** journal post.

### Stripe consume → suggest (one-shot)

`POST /api/v1/bank/stripe/reconcile-suggest` with `{company, confirm_consume?, limit?}`. Runs queue consume then reconcile suggest. Default dry-run consume; `confirm_consume:true` persists `queue.consumed` then suggests. Helper: `suggest_from_stripe_consume`. **No** auto journal post.

MCP: `bank_stripe_reconcile_suggest` — same args (`company`, `confirm_consume?`, `limit?`, `raise_exceptions?`, actor). AuthZ + allowlist. Still **no** journal post.

### Stripe consume → apply preview (one-shot opt-in)

`POST /api/v1/bank/stripe/reconcile-apply-preview` with `{company, confirm_consume?, force?, limit?}`. Consume → suggest → when **exactly one** best suggestion is ≥ `SAFE_THRESHOLD_BPS`, runs `apply_match` and issues a ConfirmStore `confirm_token` (same path as reconcile apply `preview:true`). Successful apply closes any open `unmatched_bank_transaction` for that bank row (same as reconcile apply). Otherwise returns suggestions with `applied: null`. Helper: `apply_preview_from_stripe_consume`. Still **no** auto journal commit.

MCP: `bank_stripe_reconcile_apply_preview` — same args (`company`, `confirm_consume?`, `force?`, `limit?`, actor). Unique safe match → ConfirmStore preview; otherwise suggestions only. Still **no** auto journal commit.

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

Or `row_index` + `rows` / CSV provider fields. Response `data.entry` is the journal JSON.

Optional `preview: true` (default `false`): after building the entry, runs the same ConfirmStore path as `POST /api/v1/journal/preview` and adds `confirm_token`, `expires_unix_ms`, and `payload_digest` so the client can commit without a separate preview call. Still does **not** post.

**Local web UI (DEV):** Bank screen has **Afstem apply (preview)** — invoice_id + row
(`date` / `text` / `amount_minor` i64), optional `force`, always `preview:true`.
Import drafts can **Udfyld afstem**. Commit via `/api/v1/journal/commit` or open
Journal. Never auto-posts. Revolut remains dormant without a Business token.

**Unsafe matches** (confidence &lt; `SAFE_THRESHOLD_BPS`): rejected unless `force: true` **and** actor is `user` (agents/system cannot force). When applied, any open `unmatched_bank_transaction` for that bank row related-id is closed.

### MCP — `bank_reconcile_apply` (force + optional ConfirmStore preview)

Same contract as HTTP apply: args `company`, `invoice_id`, `row` / `row_index`, optional `force`, optional `preview` (default `false`). Below `SAFE_THRESHOLD_BPS`, `force: true` is accepted **only** when `actor_kind` is `user`; agents/system are rejected. Response includes `entry`, `confidence_bps`, `forced`, `exception_closed`. With `preview: true`, also `confirm_token`, `expires_unix_ms`, `payload_digest` (same ConfirmStore path as `journal_post_preview`) so the client can `journal_post_commit` without a separate preview. Still does **not** post.

## Revolut OAuth (start / callback / refresh)

HTTP: `GET /api/v1/revolut/oauth/start`, `POST …/callback`, `POST …/refresh` (ADR-008).

MCP parity:

| Tool | Mirrors | Notes |
|------|---------|-------|
| `revolut_oauth_start` | GET start | Returns `auth_url` + `state`; never `client_secret` / tokens |
| `revolut_oauth_callback` | POST callback | Args `company` + `code`; stores `secrets/revolut.json` (`0600`); response `{stored, path}` only |
| `revolut_oauth_refresh` | POST refresh | Fail-closed without refresh_token; never echoes tokens |

Env: `KLARBOG_REVOLUT_CLIENT_ID` / `_CLIENT_SECRET` / `_REDIRECT_URI` (optional `_TOKEN_URL` / `_AUTH_URL`). Missing config → fail closed.
