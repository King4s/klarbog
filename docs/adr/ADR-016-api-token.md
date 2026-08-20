# ADR-016: Gated API bearer token

## Status
Accepted (wave 41 — auth residual #1; **not** full OIDC)

## Context
Actor headers (`x-klarbog-actor-kind` / `x-klarbog-actor-id`) remain the
ledger identity model (policy.json). On any network that can reach the API they
are spoofable. Operators asked for a stronger gate before non-loopback / shared
host exposure, without yet adopting a full IdP.

## Decision

1. **Optional shared secret:** if `KLARBOG_API_TOKEN` is set to a non-empty
   value, every request under `/api/v1/*` (except Stripe webhook ingress) must
   present that secret via:
   - `Authorization: Bearer <token>`, or
   - `x-klarbog-api-token: <token>`
2. **Unset / empty token → DEV behavior unchanged** (no bearer required).
3. **Constant-time compare** of presented vs expected token (reject on length
   mismatch without leaking which).
4. **Exempt paths** (no bearer):
   - `/health`
   - `/ui` and `/ui/*` (static UI; the browser still sends the token on API
     `fetch` from settings when configured)
   - `POST /api/v1/webhooks/stripe` (authenticated by Stripe HMAC /
     `KLARBOG_STRIPE_WEBHOOK_SECRET` — separate trust path)
5. **Actor headers remain required** on mutating/company routes as today.
   The bearer proves “caller knows the install secret”; actors still authorize
   within the company policy.
6. **OIDC** remains a later residual. Optional same-origin **session cookie**
   is ADR-017 (requires `KLARBOG_SESSION_SECRET` in addition to this token).

## Consequences
- `AppState` carries `api_token: Option<Arc<str>>` from env at process start.
- Middleware fails closed with **401** + envelope error when the token is
  configured and missing/wrong.
- UI settings gain an optional API token field (localStorage only; never logged).
- MCP stdio is unchanged (not HTTP); document that HTTP clients need the bearer
  when the env is set.
- Never commit real token values; `.env.example` stays commented.
