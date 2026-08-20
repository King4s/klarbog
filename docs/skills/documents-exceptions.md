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
- Slice 7: JSON metadata in company dir; optional binary via `content` / HTTP `content_base64` (ADR-007).

## Plugin facts

- Crate: `klarbog-plugin-documents`
- Files: `<company>/documents.json`, `<company>/exceptions.json`
- Capabilities: `Read`, `CrmWrite` — **no** `JournalWrite`
- `path_hint` must be relative (pathguard rejects absolute paths)

## HTTP (DEV)

### Attach document

`POST /api/v1/documents`

```json
{
  "company": "/path/to/company",
  "kind": "receipt",
  "path_hint": "2026/01/receipt-001.pdf",
  "party_id": "pty_...",
  "invoice_id": null,
  "notes": "Fuel station",
  "content_base64": "<optional base64 bytes>"
}
```

`kind`: `receipt` | `invoice_scan` | `other`.

### List documents

`GET /api/v1/documents?company=<path>`

### Raise exception

`POST /api/v1/exceptions`

```json
{
  "company": "/path/to/company",
  "severity": "warn",
  "code": "missing_receipt",
  "message": "No receipt for expense 6000-42",
  "document_id": null,
  "party_id": "pty_..."
}
```

`severity`: `info` | `warn` | `error`.

### List / get / close

- `GET /api/v1/exceptions?company=<path>`
- `GET /api/v1/exceptions?company=<path>&id=<exc_id>`
- `PATCH /api/v1/exceptions` with `{"company", "id", "open": false}` to close

## Rust (in-process)

```rust
use klarbog_plugin_documents::{
    attach_document, raise_exception, set_exception_open,
    DocumentKind, ExceptionSeverity,
};

let doc = attach_document(
    company,
    DocumentKind::Receipt,
    "scans/x.pdf",
    Some(party_id),
    None,
    None,
    Some(pdf_bytes),
)
.await?;
let exc = raise_exception(company, ExceptionSeverity::Warn, "missing_receipt", "...", None, Some(party_id))?;
set_exception_open(company, &exc.id, false)?;
```

## ObjectStore (ADR-007)

- `klarbog-storage`: `LocalFsStore` for DEV; `R2Store` for stage/prod (EU, fail-closed without creds).
- When `content` is provided, bytes are stored at `path_hint` under `<company>/objects/` (local) or R2 bucket key.
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
4. Close exceptions when evidence is attached or issue resolved.
5. After grace: preview `retention_purge` before `confirm: true`.
