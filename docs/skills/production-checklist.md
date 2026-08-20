# Skill: production checklist (Klarbog)

Operator gates before treating a Klarbog install as more than DEV. Klarbog
remains **DEV-default** (loopback). This checklist does **not** authorize a
public cloud rollout by itself.

Follow with [`live-e2e.md`](live-e2e.md) when payment/storage keys are available.
Never print or commit secrets.

## 1. Bind posture (ADR-003 / ADR-014)

- [ ] Default: unset `KLARBOG_BIND` → `127.0.0.1:3195`
- [ ] If non-loopback is required: **both** `KLARBOG_ALLOW_NON_LOOPBACK=1` **and**
      explicit `KLARBOG_BIND=host:port`
- [ ] Confirm process **refuses** non-loopback bind when the allow flag is off
- [ ] Prefer loopback + reverse proxy over `0.0.0.0` whenever possible

## 2. TLS edge (ADR-015)

- [ ] HTTPS terminates at a reverse proxy / load balancer in front of Klarbog
- [ ] Upstream to Klarbog is plain HTTP on loopback (or a private network)
- [ ] Clients reach **both** `/api/v1/*` and `/ui/` on the **same HTTPS origin**
- [ ] Do **not** expose non-loopback Klarbog on cleartext HTTP to untrusted networks
- [ ] Certificates and renewals are operator-owned (not inside `klarbog-api`)

## 3. Auth reality check (still DEV-shaped)

- [ ] Understand that `x-klarbog-actor-kind` / `x-klarbog-actor-id` are **not** a
      production IdP (spoofable on any network that can reach the API)
- [ ] Company paths stay under `KLARBOG_ALLOWLIST_ROOT`
- [ ] Actors used by UI/API exist in the company’s `policy.json`
- [ ] Accept residual risk until a later session/OIDC gate exists — or keep the
      API loopback-only / VPN-only

## 4. Data isolation and backup

- [ ] `KLARBOG_ALLOWLIST_ROOT` points at the intended data root
- [ ] Operator backup plan for company SQLite + documents (CLI backup helpers are
      DEV aids, not a DR SLA)
- [ ] Test restore on a scratch allowlist before relying on backups

## 5. Integrations

- [ ] Revolut / Stripe / R2 secrets live only in environment or a secret store —
      never in git
- [ ] Optional: `./scripts/live-e2e.sh` with keys → expect live smoke (not
      `LIVE_E2E_SKIP`)
- [ ] Offline gates still green: `./scripts/verify.sh` → `VERIFY_OK`;
      `./scripts/contract-smoke.sh` → `CONTRACT_SMOKE_OK`

## 6. Explicitly not production-ready yet

| Gap | Notes |
|-----|--------|
| In-process TLS | By design absent; use ADR-015 edge |
| OIDC / session auth | Actor headers only |
| Managed HA / multi-node | Single-process DEV model |
| Public deploy runbook | Out of scope of bind gate + TLS docs |

## Quick references

- [ADR-014 production posture](../adr/ADR-014-production-posture.md)
- [ADR-015 TLS edge](../adr/ADR-015-tls-edge.md)
- [ADR-013 live E2E](../adr/ADR-013-live-e2e.md)
- [INSTALL](../INSTALL.md)
