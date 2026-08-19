# ADR-009: Stripe API (payments rail)

## Status
Accepted (corrected 2026-08-20 — **API is primary**, not CSV)

## Decision
Klarbog integrates **Stripe** via the **Stripe API** as a first-class payments rail (balance transactions, payouts, charges as needed).

### Primary: API
- Fetch with restricted/secret key from env:
  - `KLARBOG_STRIPE_SECRET_KEY` (secret — never commit/print)
  - optional `KLARBOG_STRIPE_API_BASE` (default `https://api.stripe.com`)
- Prefer balance transactions / payouts → `BankRow`-compatible drafts (`MinorAmount` i64, no `f64`).
- Prefer **net** when fee is present; currency fail-closed vs company default.
- Capability **Read** — drafts only; post via journal two-phase confirm.
- DEV/tests: fixture JSON / mocked HTTP — no network in verifier.

### Secondary: CSV (optional offline)
- `BankProfile::Stripe` CSV remains a **manual fallback** only.
- Product AI prompt: Stripe = API; CSV = nød/offline.

### Webhooks (implemented)
- Ingress: `POST /api/v1/webhooks/stripe` — HMAC verify (`KLARBOG_STRIPE_WEBHOOK_SECRET`), append to `{company}/stripe_webhooks/queue.jsonl`, draft rows for `payout.paid` / `charge.succeeded`.
- Consume: `POST /api/v1/bank/stripe/consume` — actor AuthZ + allowlist; default dry-run / `confirm:true` fail-closed (like retention purge); idempotent via `queue.consumed` sidecar (event id); returns count + row preview. **No journal post**.
- Pipeline: `POST /api/v1/bank/stripe/reconcile-suggest` — `suggest_from_stripe_consume` (consume drafts → reconcile suggest in one call). Body `{company, confirm_consume?, limit?}`; dry-run consume by default; `confirm_consume:true` persists then suggests. Still **no** auto journal post.
- Pipeline: `POST /api/v1/bank/stripe/reconcile-apply-preview` — `apply_preview_from_stripe_consume` (consume → unique safe apply + ConfirmStore preview). Body `{company, confirm_consume?, force?, limit?}`. Applies only when exactly one best suggestion ≥ safe threshold; successful apply closes open `unmatched_bank_transaction` for that row (same as reconcile apply); else suggestions only. Still **no** auto journal commit.

### Out of scope here
- Connect / OAuth onboarding UI. Initial DEV uses server-side secret key from env.

## Consequences
HTTP/MCP bank import gains `source: "api"` + `provider: "stripe"`.
Webhook queue + consume path is Read/draft only; posting remains two-phase journal confirm.
