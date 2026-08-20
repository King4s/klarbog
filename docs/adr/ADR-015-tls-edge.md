# ADR-015: TLS at the reverse-proxy edge

## Status
Accepted (wave 40 — prod residual #1; **no in-process TLS**)

## Context
[ADR-014](ADR-014-production-posture.md) allows an explicit non-loopback bind but
leaves TLS out of `klarbog-api`. Binding plain HTTP on a LAN/WAN address exposes
actor headers, company paths, and any payment secrets in transit.

Operators asked for a clear production posture: how to put TLS in front without
changing Klarbog’s DEV-default loopback model.

## Decision

1. **`klarbog-api` stays plain HTTP** (loopback or gated bind). No rustls /
   HTTPS listener inside the binary in this track.
2. **TLS terminates at an operator-owned reverse proxy** (Caddy, nginx, Traefik,
   cloud load balancer, etc.) that speaks HTTPS to clients and proxies to
   Klarbog’s bind address (typically `127.0.0.1:3195` on the same host, or a
   private upstream).
3. **Never publish non-loopback Klarbog without a TLS edge.** If
   `KLARBOG_ALLOW_NON_LOOPBACK=1` is set, treat a TLS reverse proxy (or
   equivalent encrypted path) as mandatory operator policy — Klarbog will not
   enforce certificate presence in-process.
4. **Same-origin UI** (`/ui/` served by the API) should be reached through the
   same HTTPS origin as the API when exposed beyond loopback, so browsers do
   not mix cleartext and TLS.

## Recommended shape

```text
Client (HTTPS)
    → reverse proxy :443 (TLS cert)
        → http://127.0.0.1:3195  (klarbog-api; KLARBOG_BIND unset or loopback)
```

Prefer keeping Klarbog on **loopback** behind the proxy. Non-loopback
`KLARBOG_BIND` is only for unusual topologies; it still requires ADR-014 flags
and still expects TLS *somewhere* toward the client.

## Out of scope
- Certificate issuance / ACME automation inside Klarbog
- mTLS between proxy and API
- Stronger application auth (OIDC / sessions) — later residual
- Public CDN or multi-region deploy

## Consequences
- Document the edge in INSTALL and
  [`docs/skills/production-checklist.md`](../skills/production-checklist.md).
- ADR-014 residual “No TLS in-process” remains intentional; this ADR is the
  operator answer, not a code change to `klarbog-api`.
