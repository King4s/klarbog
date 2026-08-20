# ADR-018: Rust server-rendered UI (replaces JS SPA)

## Status
Accepted (2026-08-20 — owner mandate: Rust end-to-end)

## Context
[ADR-012](ADR-012-local-web-ui.md) shipped a Klarbog-owned **static** HTML/CSS/JS
DEV UI served from `klarbog-api`. Owner decision 2026-08-20: the product UI must
be **Rust all the way** — no JS SPA / no `ui/js` application logic. Agents run
this migration autonomously.

## Decision

### Server-rendered HTML from `klarbog-api`
- UI is **Askama** templates compiled into `klarbog-api`, routed under `/ui/*`.
- Handlers call the same domain logic / plugins as JSON API (AuthZ + allowlist).
- Money remains **i64** minor units in handlers; templates only format for display.
- Optional static assets limited to **non-logic** files (e.g. `styles.css`).
  No application JavaScript modules.

### Supersedes JS progressive SPA
- ADR-012 progressive JS screens and the wave-51 `ui/js/*` soft-split are
  **retired** as the product UI path.
- JSON `/api/v1/*` and MCP remain for agents; humans use `/ui/*` HTML.

### Loopback / isolation unchanged
- ADR-003 / ADR-014 bind and production gates still apply. This ADR does not
  authorize public deploy.

## Consequences
- `mount_ui` serves Askama routes (plus optional CSS), not an SPA `index.html`.
- New screens land as Rust handlers + templates, not `ui/js/*.js`.
- Offline VERIFY continues to cover HTML smoke (brand + key routes).
