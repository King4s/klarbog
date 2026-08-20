//! Retention, GDPR, invoice, and journal MCP tool schemas.

use serde_json::{json, Value};

pub fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "retention_get",
            "description": "Load company retention policy (read-only)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "retention_purge",
            "description": "Purge closed exceptions (and optional orphan document metadata); dry-run unless confirm:true; journal untouched",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "confirm": {"type": "boolean"},
                    "gc_orphan_documents": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "backup_manifest",
            "description": "Write backup manifest + sha256 sidecar; returns paths and digests",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "gdpr_export",
            "description": "Write company-scoped GDPR export v1 metadata (gdpr_export.json; no binary blobs; i64 totals; journal note)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "gdpr_erase_party",
            "description": "GDPR party erase preview/confirm: anonymize display_name; strip or delete docs; journal immutable (journal_refs_retained)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "party_id": {"type": "string"},
                    "confirm": {"type": "boolean"},
                    "delete_documents": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "party_id", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_mark_paid_preview",
            "description": "Mark invoice paid for remaining balance (payment ledger) and return payment journal suggestion with party_id (no post). Rejects when remaining is 0.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "invoice_id", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_mark_part_paid_preview",
            "description": "Record partial payment on invoice ledger (amount_minor >0 and < remaining), set part_paid, return journal suggestion with party_id (no post). Overpay rejected.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "amount_minor": {"type": "integer"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "invoice_id", "amount_minor", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "journal_post_preview",
            "description": "Phase-1: validate entry and return confirmation token (no write)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "entry": {"type": "object"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "entry", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "journal_post_commit",
            "description": "Phase-2: commit with confirmation token (ADR two-phase confirm)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "entry": {"type": "object"},
                    "confirm_token": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "entry", "confirm_token", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "journal_moms_post_suggestion",
            "description": "Optional moms post suggestion from gross_minor when memo has #vat25 (net+vat i64 legs). Preview only — never posts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "gross_minor": {"type": "integer"},
                    "memo": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "gross_minor", "memo", "actor_kind", "actor_id"]
            }
        }),
    ]
}
