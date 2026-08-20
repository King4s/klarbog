---
name: klarbog-gdpr-erase-party
description: >-
  GDPR party erasure via MCP/HTTP/CLI. Anonymize CRM display_name; never mutate
  confirmed journal (journal_refs_retained).
---

# GDPR erase party

## When to use

- Erase personal data for a CRM party after a deletion request.
- Preview first (`confirm` omitted/false), then confirm.

## Related: company export

Before erase, agents may call MCP `gdpr_export` (mirrors
`POST /api/v1/gdpr-export` / CLI `klarbog gdpr-export`) for company-scoped
metadata (`parties`, `invoices` with i64 `total_minor`, `documents`,
`exceptions`, retention summary + immutable-journal note). Writes
`gdpr_export.json` — **no** binary blobs.

## Immutable journal

Confirmed journal entries are **never** rewritten. Party legs stay as historical
references. The erase report always includes `journal_refs_retained` (entry ids
that still mention the party).

## MCP

Tool: `gdpr_erase_party`

Args: `company`, `party_id`, optional `confirm` / `delete_documents`, plus
`actor_kind` / `actor_id`. AuthZ + company allowlist like other MCP tools.

Default: dry-run (anonymize preview only). `confirm: true` applies
`display_name` → `erased` and either strips document `party_id` or deletes
documents when `delete_documents: true`.

## Agent checklist

1. Call without `confirm` first; inspect `journal_refs_retained`.
2. Do not attempt to scrub the ledger — retention/accounting wins.
3. Only confirm after the operator accepts the retained journal refs.
