# ADR-014: Production posture (gated bind; loopback default)

## Status
Accepted (wave 39 — production track scaffold; **no public deploy**)

## Context
Owner wants a path toward production after UI and live E2E. Klarbog remains
**DEV-default** under ADR-003 (loopback `:3195`). Exposing the HTTP API on a
non-loopback interface is an explicit, opt-in escape hatch — never the default
and not a public deploy authorization.

## Decision

### Default remains loopback (ADR-003)
- `klarbog-api` binds **`127.0.0.1:3195`** unless configured otherwise.
- Unset / empty `KLARBOG_BIND` → loopback default.

### Optional non-loopback bind (fail-closed)
Non-loopback listen is allowed **only** when **both**:

1. `KLARBOG_ALLOW_NON_LOOPBACK=1` (truthy: `1` / `true` / `yes`), **and**
2. Explicit `KLARBOG_BIND=<host:port>` (e.g. `0.0.0.0:3195`).

Otherwise:
- Loopback `KLARBOG_BIND` values are always accepted.
- Any non-loopback `KLARBOG_BIND` **without** the allow flag → **refuse at
  startup** (fail-closed). The allow flag alone does **not** change the default
  bind.

### Out of scope for this ADR
- Public CDN / internet-facing deploy
- Automatic TLS termination inside `klarbog-api`
- Strong auth beyond existing actor / allowlist headers
- Managed backup/restore SLA

## Residual risks (intentional)

| Risk | Why it remains |
|------|----------------|
| **No TLS in-process** | Binding non-loopback without a reverse proxy / TLS edge exposes plain HTTP (credentials, actor headers, company paths). Operators must terminate TLS elsewhere — see [ADR-015](ADR-015-tls-edge.md). |
| **Auth is still DEV-shaped** | Actor headers + path allowlist are not a production identity system (no OIDC session gate, CSRF hardening for browsers on shared networks, etc.). |
| **Backups** | Company SQLite / object stores need an operator-owned backup plan; Klarbog CLI backup helpers are DEV aids, not a production DR guarantee. |
| **Mis-set env** | `KLARBOG_ALLOW_NON_LOOPBACK=1` + broad `KLARBOG_BIND` can expose the API on LAN/WAN. Never enable by default; document in INSTALL / `.env.example` as commented-out only. |

## Consequences
- `klarbog-api` parses bind env in the binary; unit tests cover the refuse path.
- INSTALL and `.env.example` note the gate; defaults stay safe.
- This is **scaffold only** — no sister-product names, no secrets, no public
  production rollout implied by merging the gate.
