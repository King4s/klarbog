# ADR-007: Cloudflare EU object storage (R2+)

## Status
Accepted (owner 2026-08-19). **SigV4 put/get implemented** (2026-08-20, `klarbog-storage`).

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

### Implementation (2026-08-20)
- Crate: `crates/klarbog-storage`
- **PutObject / GetObject** via `reqwest` + manual **AWS SigV4** (`sigv4.rs`), region `auto`, path-style URI against `R2Config.endpoint`.
- Unit tests: signing vectors + key/jurisdiction guards; **no live R2 in CI**.
- **DeleteObject**: still deferred (`NotImplementedInDev`); add same signer when needed.

### Optional next steps
1. Wire document attach to call `R2Store::put` when `KLARBOG_STORAGE=r2`.
2. Implement `delete` with signed DELETE.
3. Stage smoke script (owner creds) + optional integration test behind `#[ignore]`.
4. Multipart upload only if objects exceed single PUT limits.

## Consequences
Slice 7 documents plugin talks to a storage trait (`Local` | `R2`). Backup/GDPR slice (9) can reuse the same store for exports.
