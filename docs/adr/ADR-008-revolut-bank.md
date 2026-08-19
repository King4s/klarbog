# ADR-008: Revolut Business API (bank rail)

## Status
Accepted (corrected 2026-08-20 — **API is primary**, not CSV)

## Decision
Klarbog integrates **Revolut** via the **Revolut Business API** as a first-class payment/bank rail.

### Primary: API
- Live fetch of account transactions (and related balances as needed) using env credentials:
  - `KLARBOG_REVOLUT_API_TOKEN` (secret — never commit/print)
  - optional `KLARBOG_REVOLUT_API_BASE` (default Revolut Business API base URL)
- Map API amounts to `MinorAmount` (i64). No `f64`.
- Currency fail-closed vs company default when configured.
- Capability **Read** — API sync produces **draft** `JournalEntry` suggestions only; ledger write remains host preview/commit.
- DEV/tests: fixture JSON / mocked HTTP — no network required for `./scripts/verify.sh`.

### Secondary: CSV (optional offline)
- Keep `BankProfile::Revolut` CSV parser only as **manual fallback** when the user has an export file and no API key.
- Product messaging and AI prompt must say: Revolut = API; CSV = nød/offline.

### Out of scope here
- Full OAuth app install UX (may follow); initial DEV may use personal/business API token from env.

### OAuth scaffold (2026-08-20)
- Env: `KLARBOG_REVOLUT_CLIENT_ID`, `KLARBOG_REVOLUT_CLIENT_SECRET`, `KLARBOG_REVOLUT_REDIRECT_URI`, optional `KLARBOG_REVOLUT_TOKEN_URL` / `KLARBOG_REVOLUT_AUTH_URL`.
- HTTP: `GET /api/v1/revolut/oauth/start` → auth URL; `POST /api/v1/revolut/oauth/callback` with `{ company, code }` → token exchange; `POST /api/v1/revolut/oauth/refresh` with `{ company }` → refresh grant (same AuthZ as start/callback; response never includes raw tokens).
- DEV token store: `{company}/secrets/revolut.json` mode `0600` (atomic write; never log secrets). Persists `access_token`, optional `refresh_token`, `expires_at` (unix ms). Env bearer `KLARBOG_REVOLUT_API_TOKEN` remains primary when set; otherwise company secrets are used and refreshed when access is expired and a refresh token is present (fail-closed if refresh is missing).

## Consequences
HTTP/MCP bank import gains `source: "api"` + `provider: "revolut"` (in addition to CSV profile).
