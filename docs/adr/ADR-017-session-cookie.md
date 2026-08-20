# ADR-017: Optional HMAC session cookie

## Status
Accepted (wave 43 — auth residual #2; **not** full OIDC)

## Context
ADR-016 gates HTTP with a shared API bearer. Browser UI callers still need to
store that secret (localStorage) or send it on every `fetch`. Operators asked
for a same-origin **session cookie** so the UI can authenticate after a one-shot
login without keeping the raw API token in JS longer than necessary — still
without adopting an IdP.

## Decision

1. **Enable when both are set (non-empty):**
   - `KLARBOG_API_TOKEN` (ADR-016), and
   - `KLARBOG_SESSION_SECRET` (HMAC key for the cookie; distinct from the API
     token).
2. **`POST /api/v1/auth/login`** with JSON `{ "token": "..." }` matching the
   API token (constant-time) → `Set-Cookie: klarbog_session=…`:
   - `HttpOnly`
   - `SameSite=Lax`
   - `Path=/`
   - `Secure` only when non-loopback posture / env flag
     (`KLARBOG_SESSION_COOKIE_SECURE=1`, or non-loopback bind with
     `KLARBOG_ALLOW_NON_LOOPBACK=1`)
3. **Cookie value:** HMAC-SHA256 signed `v1.<exp_unix>.<hex_sig>` over
   payload `v1.<exp_unix>` using `KLARBOG_SESSION_SECRET`. Reject expired or
   tampered cookies.
4. **Middleware:** when the API token gate is on, accept **either** a valid
   Bearer / `x-klarbog-api-token` **or** a valid `klarbog_session` cookie.
5. **`POST /api/v1/auth/logout`** clears the cookie (`Max-Age=0`).
6. **Unset / empty `KLARBOG_SESSION_SECRET` → no session routes and no cookie
   auth path** (Bearer-only as ADR-016). Login/logout are not mounted.
7. **Explicitly out of scope:** OIDC IdP, CSRF tokens for cross-site POSTs,
   multi-user accounts / per-actor sessions.

## Consequences
- `AppState` carries `session_secret` + `session_cookie_secure` from env at
  process start.
- UI may call login once and rely on the cookie for same-origin API calls.
- Operators must treat `KLARBOG_SESSION_SECRET` like any other install secret;
  rotating it invalidates existing cookies.
- Residual: same-site cookie auth is not a substitute for TLS at the edge
  (ADR-015) or a real IdP.
