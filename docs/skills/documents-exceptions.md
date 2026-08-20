---
name: klarbog-documents-exceptions
description: >-
  Attach document metadata and raise/close exceptions for a Klarbog company.
  Binary storage via ObjectStore (local/R2 scaffold); no journal write.
---

# Documents + exceptions

## When to use

- Register receipt/scan metadata linked to `party_id` or `invoice_id`.
- Raise workflow exceptions (missing receipt, amount mismatch) and close when resolved.
- Slice 7: JSON metadata in company dir; optional binary via `content` / HTTP/MCP `content_base64` (ADR-007).

## Plugin facts

- Crate: `klarbog-plugin-documents`
- Files: `<company>/documents.json`, `<company>/exceptions.json`
- Capabilities: `Read`, `CrmWrite` — **no** `JournalWrite`
- `path_hint` must be relative (pathguard rejects absolute paths and `..`)

## MCP (parity with HTTP)

| Tool | Mirrors |
|------|---------|
| `documents_attach` | `POST /api/v1/documents` |
| `documents_list` | `GET /api/v1/documents` (`document_id` optional get) |
| `documents_delete` | `DELETE /api/v1/documents` (`delete_object` default true) |
| `exceptions_raise` | `POST /api/v1/exceptions` |
| `exceptions_list` | `GET /api/v1/exceptions` (`exception_id` optional get; `open_only` default true) |
| `exceptions_set_open` | `PATCH /api/v1/exceptions` (`open:false` closes) |

Common args: `company`, `actor_kind`, `actor_id`. AuthZ + allowlist. **Never** posts journal.

### Attach example (MCP)

```json
{
  "company": "/path/to/company",
  "kind": "receipt",
  "path_hint": "2026/01/receipt-001.pdf",
  "party_id": "pty_...",
  "notes": "Fuel station",
  "content_base64": "<optional base64 bytes>",
  "actor_kind": "user",
  "actor_id": "owner"
}
```

`kind`: `receipt` | `invoice_scan` | `other`.

### Raise example (MCP)

```json
{
  "company": "/path/to/company",
  "code": "missing_receipt",
  "severity": "warn",
  "message": "No receipt for expense 6000-42",
  "related_ids": ["doc_..."],
  "actor_kind": "user",
  "actor_id": "owner"
}
```

`severity`: `info` | `warn` | `error`.

## HTTP (DEV)

Same bodies as MCP (without actor fields — use `x-klarbog-actor-*` headers).

- `POST` / `GET` / `DELETE` `/api/v1/documents`
- `POST` / `GET` / `PATCH` `/api/v1/exceptions`

## Rust (in-process)

```rust
use klarbog_plugin_documents::{
    attach_document, raise_exception, set_exception_open,
    DocumentKind, ExceptionSeverity,
};

let doc = attach_document(
    company,
    DocumentKind::Receipt,
    "scans/x.pdf".into(),
    Some(party_id),
    None,
    None,
    Some(pdf_bytes),
)
.await?;
let exc = raise_exception(
    company,
    "missing_receipt".into(),
    ExceptionSeverity::Warn,
    "No receipt".into(),
    vec![doc.id.to_string()],
)?;
set_exception_open(company, &exc.id, false)?;
```

## ObjectStore (ADR-007)

- `klarbog-storage`: `LocalFsStore` for DEV; `R2Store` for stage/prod (EU, fail-closed without creds).
- When `content` / `content_base64` is provided, bytes are stored at `path_hint` under `<company>/objects/` (local) or R2 bucket key.
- Agents should use relative `path_hint`; host resolves against company storage root.

## Retention purge (MCP / HTTP)

Closed exceptions (and optional orphan document **metadata**) can be purged after
the company retention grace period — see ADR-010. Surfaces:

- HTTP: `POST /api/v1/retention/purge` (`confirm`, optional `gc_orphan_documents`)
- MCP: `retention_purge` (same args; AuthZ + allowlist)

Default is dry-run. Purge never touches confirmed journal entries.

## Agent checklist

1. Ensure party (and invoice if linked) exist.
2. Use relative `path_hint` only.
3. Raise exceptions for human review — do not auto-post journal fixes.
4. Close exceptions when evidence is attached or issue resolved (`exceptions_set_open` with `open:false`).
5. After grace: preview `retention_purge` before `confirm: true`.
