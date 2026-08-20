# ADR-013: Live E2E track (optional keys, loopback)

## Status
Accepted (wave 38 — live E2E scaffold; amended 2026-08-20 — Revolut dormant)

## Context
Klarbog payment rails (Revolut, Stripe) and R2 storage need occasional **live**
smoke with real DEV credentials. Offline `contract-smoke` and `verify.sh` must
stay green without secrets. Production bind is still out of scope.

Owner may not yet have a Revolut **Business** account. Revolut live preview must
not block Stripe + R2 live smoke.

## Decision

### Fail-closed SKIP without Stripe + R2
- `./scripts/live-e2e.sh` requires
  `KLARBOG_STRIPE_SECRET_KEY` and `KLARBOG_R2_*`.
- If any of those is unset/empty → print a clear **SKIP**, exit **0**,
  and make **no** live provider/R2 calls.

### Revolut optional (dormant)
- `KLARBOG_REVOLUT_API_TOKEN` is **optional**.
- When unset: skip Revolut `import/preview`, print an explicit DORMANT/SKIP line,
  continue with Stripe. Do not invent tokens or call Revolut.
- When set: run Revolut `import/preview` as before.

### Bounded loopback smoke when Stripe + R2 are present
- Target **127.0.0.1:3195** only (reuse running API or start one).
- Smoke: `/health`, `/api/v1/status`, bank `import/preview` with `source=api`
  for Stripe (and Revolut when token set). Drafts only; no journal post.
- Never print secret values; never commit secrets.

### Separation from prod
- This ADR does **not** authorize non-loopback bind, public hosting, or prod
  deploy. Prod remains a later owner track after live E2E is trusted.

## Consequences
- CI / default agent runs keep using offline gates.
- Owners export secrets in the shell that runs `live-e2e.sh` (or a gitignored
  `.env` loaded manually).
- Revolut Business onboarding unblocks the Revolut preview leg later; no code
  change required beyond exporting the token.
- Skill: [`docs/skills/live-e2e.md`](../skills/live-e2e.md).
