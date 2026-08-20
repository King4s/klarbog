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

## Two-phase commit pattern (added 2026-08-21)
The confirm token from `journal_preview` is bound to a digest of the exact
`JournalEntry` (including `as_of`). SSR pages must therefore **never rebuild**
the entry from form fields at commit time — a fresh `as_of` changes the digest
and the token fails closed ("payload mismatch"; this shipped as a live-only bug
in the journal page). Pattern for every preview→commit flow (journal, invoice
payment, bank apply):
1. Preview serializes the exact entry to JSON into a hidden `entry_json` field
   alongside `confirm_token`.
2. Commit deserializes `entry_json` and passes it to `journal_commit`
   unchanged. Serde round-trip preserves the digest
   (`ui_entry_json_roundtrip_preserves_digest`).
3. Tampering with `entry_json` is harmless: the digest no longer matches the
   token and commit fails closed.

`scripts/ui-smoke.sh` (CI: `rust-dev.yml`) exercises all three flows against a
real server precisely because unit tests missed the rebuild bug.
