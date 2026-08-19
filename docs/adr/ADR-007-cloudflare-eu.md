# ADR-007: Cloudflare EU object storage (R2+)

## Status
Accepted (owner 2026-08-19)

## Decision
Klarbog may use **Cloudflare products that offer EU data residency / EU-approved processing**, starting with:

1. **R2** — S3-compatible object storage for documents, backups, and invoice attachments (slice 7+).
2. Later candidates (only when needed): Workers (edge compute with EU routing where available), Queues, D1 — each requires an explicit follow-up ADR for residency + DPIA notes.

### Hard rules
- **Default DEV backend remains local filesystem** under the company allowlist path.
- Production/stage R2 uses env-only credentials (`KLARBOG_R2_*`); **never** commit secrets.
- Prefer **EU jurisdiction** bucket / location hints (e.g. `WEUR` / EU endpoint). Reject configs that point at non-EU endpoints unless owner overrides with an explicit `KLARBOG_R2_ALLOW_NON_EU=1` (fail-closed default).
- Documents store **object keys + metadata** in company JSON; binaries live in the configured `ObjectStore`.
- No Cloudflare dependency for ledger correctness — storage is an attachment plane.

## Consequences
Slice 7 documents plugin talks to a storage trait (`Local` | `R2`). Backup/GDPR slice (9) can reuse the same store for exports.
