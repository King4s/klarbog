# ADR-012: Local DEV web UI

## Status
**Superseded** by [ADR-018](ADR-018-rust-ssr-ui.md) (2026-08-20 — Rust SSR UI).
Historical acceptance: wave 36 JS static UI track.

## Context
Klarbog already exposes loopback HTTP (`klarbog-api`), MCP, and CLI. Humans still
need a thin GUI for day-to-day DEV inspection. The product must stay Klarbog-
owned: screens call the existing REST API; this is **not** a port of an upstream
React cockpit.

**Amendment (2026-08-20):** owner requires **Rust end-to-end**. The JS SPA under
`ui/` / `ui/js/` is retired in favor of Askama templates in `klarbog-api`
(ADR-018). The decisions below remain historically true for the JS track.

## Decision

### Same-origin static UI from `klarbog-api`
- Serve a **DEV local web UI** from `klarbog-api` on the **same origin** as REST
  (`/api/v1/*`).
- Static assets live under repo directory `ui/` (mounted at `/ui/` or equivalent).
- No separate UI process in v1.

### Klarbog-owned screens
- UI is first-party Klarbog HTML/CSS/JS (or later Klarbog-owned tooling) that
  calls the HTTP API.
- Do **not** port an upstream React cockpit or reuse sister-product UI trees,
  routes, or component catalogs.

### Loopback + isolation (ADR-003)
- UI is **loopback / DEV only**, same binding constraints as the API.
- This ADR does **not** authorize non-loopback hosting, public CDN UI, or prod
  bind.

### Money display (ADR-001)
- API amounts remain **i64 minor units**.
- UI formats for humans via **øre / DKK helpers** (display only); never introduce
  `f32`/`f64` money math in the UI path.

### Progressive slices
1. **First:** status, parties, invoices.
2. **Later:** journal preview / confirm UX (and bank surfaces after that).
3. **Wave 44:** journal **moms-forslag** UI wired to
   `POST /api/v1/journal/moms-suggest` (preview only; ADR-011).
4. **Wave 51:** static UI soft-split into ES modules under `ui/js/` (shared
   helpers + one file per screen); `ui/app.js` is the router shell only.

## Consequences
- `klarbog-api` gains a static-file serve path for `ui/` (env override optional;
  default to repo `ui/`).
- Visual language must stay **Klarbog-owned** and clearly distinct from the
  sister product’s warm **paper / terracotta** cockpit look (no shared theme
  tokens or “accounting paper” skin). Prefer a cool mist + deep teal ink
  direction with **Klarbog** as the primary first-viewport brand signal.
- Live E2E and production tracks remain separate owner waves after this UI track
  lands slice-by-slice (live E2E scaffold: ADR-013).
- ROADMAP wave 36 stubs the progressive UI slices; journal preview UX is
  explicitly deferred.
