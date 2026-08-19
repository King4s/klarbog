# ADR-003: DEV-only hard isolation

## Status
Accepted

## Decision
- Klarbog runs **DEV-only** by default (loopback API `:3195`, local MCP stdio).
- Verifier **rejects** dirty paths outside the configured allowlist root
  (`ALLOWLIST_ROOT` / `KLARBOG_ALLOWLIST_ROOT`, default: repository root).
- Swarm workers must not run deploy, docker mutate, or arbitrary network writes
  outside that allowlist.

## Consequences
Keeps autonomous worktrees from escaping the Klarbog checkout.
