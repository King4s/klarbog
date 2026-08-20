# ADR-013: Live E2E track (optional keys, loopback)

## Status
Accepted (wave 38 — live E2E scaffold)

## Context
Klarbog payment rails (Revolut, Stripe) and R2 storage need occasional **live**
smoke with real DEV credentials. Offline `contract-smoke` and `verify.sh` must
stay green without secrets. Production bind is still out of scope.

## Decision

### Fail-closed SKIP without keys
- `./scripts/live-e2e.sh` checks
  `KLARBOG_REVOLUT_API_TOKEN`, `KLARBOG_STRIPE_SECRET_KEY`, and `KLARBOG_R2_*`.
- If any required secret is unset/empty → print a clear **SKIP**, exit **0**,
  and make **no** live provider/R2 calls.

### Bounded loopback smoke when keys are present
- Target **127.0.0.1:3195** only (reuse running API or start one).
- Smoke: `/health`, `/api/v1/status`, bank `import/preview` with `source=api`
  for Revolut and Stripe (drafts only; no journal post).
- Never print secret values; never commit secrets.

### Separation from prod
- This ADR does **not** authorize non-loopback bind, public hosting, or prod
  deploy. Prod remains a later owner track after live E2E is trusted.

## Consequences
- CI / default agent runs keep using offline gates.
- Owners export secrets in the shell that runs `live-e2e.sh` (or a gitignored
  `.env` loaded manually).
- Skill: [`docs/skills/live-e2e.md`](../skills/live-e2e.md).
