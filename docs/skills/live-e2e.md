---
name: klarbog-live-e2e
description: >-
  Optional live E2E smoke against loopback klarbog-api when Revolut/Stripe/R2
  secrets are set. Fail-closed SKIP without keys. Never commit or print secrets.
---

# Live E2E (DEV loopback)

## When to use

- After UI / offline contract smoke are green, and the owner has real DEV keys.
- To exercise **live** Revolut + Stripe `source=api` import preview through
  loopback HTTP (`127.0.0.1:3195`), plus `/health` and `/api/v1/status`.
- **Not** for CI by default — CI stays offline (`verify.sh`, `contract-smoke.sh`).

## Fail-closed without keys

`./scripts/live-e2e.sh` exits **0** with `LIVE_E2E_SKIP` when any of these are
unset or empty:

- `KLARBOG_REVOLUT_API_TOKEN`
- `KLARBOG_STRIPE_SECRET_KEY`
- `KLARBOG_R2_ACCOUNT_ID`
- `KLARBOG_R2_ACCESS_KEY_ID`
- `KLARBOG_R2_SECRET_ACCESS_KEY`
- `KLARBOG_R2_BUCKET`

No network calls to payment rails or R2 happen in the SKIP path.

## With keys set

1. Ensures `KLARBOG_ALLOWLIST_ROOT` (default: repo root) and starts
   `klarbog-api` on **127.0.0.1:3195** if `/health` is not already up.
2. Sets `KLARBOG_STORAGE=r2` by default (override allowed).
3. Creates a temp company, then:
   - `GET /health`
   - `GET /api/v1/status`
   - `POST /api/v1/bank/import/preview` with `source=api` for `revolut` and
     `stripe` (drafts only — **no journal post**)
4. Prints counts / OK lines only — **never** prints token or key values.
5. Ends with `LIVE_E2E_OK` on success.

Optional: `KLARBOG_API_BASE` (default `http://127.0.0.1:3195`),
`LIVE_E2E_CURL_MAX_TIME` (seconds).

## Hard rules

- Never commit `.env` or filled secrets.
- No sister-product env names or UI trees.
- No production / non-loopback bind (ADR-003 / ADR-013).
- Offline gates remain authoritative for merge: `./scripts/verify.sh` →
  `VERIFY_OK`.

## See also

- [`docs/INSTALL.md`](../INSTALL.md) — install + live E2E section
- [`docs/adr/ADR-013-live-e2e.md`](../adr/ADR-013-live-e2e.md)
- [`bank-import.md`](bank-import.md), [`storage-r2.md`](storage-r2.md)
