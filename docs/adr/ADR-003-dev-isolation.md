# ADR-003: DEV-only hard isolation

## Status
Accepted (Claude CONDITIONAL GO CRITICAL-3)

## Decision
- Klarbog runs **DEV-only** (loopback API `:3195`, local MCP stdio).
- Verifier **rejects** any dirty/committed path outside `/opt/pellucid-software/klarbog/`.
- Swarm workers must not run deploy, docker mutate, or arbitrary network writes.
- No SeaAid/Stripe/prod changes from this repo's automation.

## Consequences
Protects live Pellucid/SeaAid hosts from autonomous night loops.
