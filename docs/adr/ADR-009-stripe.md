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

### Out of scope here
- Connect / OAuth onboarding UI; webhooks (later ADR). Initial DEV uses server-side secret key from env.

## Consequences
HTTP/MCP bank import gains `source: "api"` + `provider: "stripe"`.
