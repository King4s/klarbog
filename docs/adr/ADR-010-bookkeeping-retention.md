# ADR-010: DK bookkeeping retention vs GDPR party erase

## Status
Accepted (slice 38)

## Context
Danish bookkeeping practice expects business records to be kept on the order of
**~5 years**. Klarbog defaults `retention.json` to `retain_days: 1825`
(~5×365). GDPR subject requests may still require erasure of personal data in
CRM and related metadata.

These duties conflict if “erase party” meant deleting every ledger fact that
mentions the party. Confirmed journal entries are the company’s accounting
truth and must remain auditable for the retention window.

## Decision

### Bookkeeping retention wins for the journal
- Confirmed journal entries are **immutable**.
- `erase_party` **never** deletes, rewrites, or strips legs from posted
  journal entries — including `party_id` on legs.
- Erasure reports **always** list retained entry ids as
  `journal_refs_retained` (dry-run and confirm).
- GDPR export carries a fixed `note` stating the same immutability rule.

### What party erase does change
- CRM: anonymize `display_name` → `erased` (party id retained as opaque key).
- Documents: strip `party_id` from metadata, or delete objects when
  `delete_documents: true`.
- Append-only erase audit (`gdpr_erase_audit.jsonl`): party id, timestamp,
  mode, docs touched — no display names or secrets.

### Payments / invoices metadata policy
- Invoice rows and their `payments[]` ledger
  (`unix_ms`, `amount_minor`, `currency`) are **business records**, not CRM
  PII payloads.
- `erase_party` does **not** delete invoices or payment rows, and does **not**
  scrub invoice `party_id` (opaque id only; the human-readable name lives in
  CRM and is anonymized).
- Backup manifests may summarize payments as `count` / `total_minor` only.
- GDPR export includes invoice metadata (id, party id, status, kind, totals)
  without binary blobs; operators rely on anonymized CRM +
  `journal_refs_retained` for subject response completeness.

### Retention purge scope
- Retention purge may remove **closed exceptions** (and optional orphan
  document metadata) after the configured grace period.
- Surfaces: `POST /api/v1/retention/purge` and MCP `retention_purge`
  (dry-run unless `confirm: true`; optional `gc_orphan_documents`).
- Purge must **not** delete confirmed journal entries or invoice/payment
  business records before `retain_days` elapses. Journal immutability under
  party erase is independent of purge.

## Consequences
- Agents and operators treat `journal_refs_retained` as expected, not as a
  failure: accounting retention overrides full ledger scrubbing.
- Subject erasure is “CRM + optional documents,” not “rewrite history.”
- Extending erase to scrub invoice `party_id` would require a separate ADR and
  must not touch the journal.
