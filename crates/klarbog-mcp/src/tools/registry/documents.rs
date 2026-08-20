//! Documents + exceptions MCP tool schemas (HTTP parity; no journal write).

use serde_json::{json, Value};

pub fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "documents_attach",
            "description": "Attach document metadata (optional content_base64). No journal write (ADR-004).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "kind": {"type": "string", "enum": ["receipt", "invoice_scan", "other"]},
                    "path_hint": {"type": "string"},
                    "party_id": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "notes": {"type": "string"},
                    "content_base64": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "kind", "path_hint", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "documents_list",
            "description": "List documents, or get one when document_id is set (read-only)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "document_id": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "documents_delete",
            "description": "Delete document metadata; delete_object defaults true. No journal write.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "document_id": {"type": "string"},
                    "delete_object": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "document_id", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "exceptions_raise",
            "description": "Raise an open exception (code/severity/message; optional related_ids). No journal write.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "code": {"type": "string"},
                    "severity": {"type": "string", "enum": ["info", "warn", "error"]},
                    "message": {"type": "string"},
                    "related_ids": {"type": "array", "items": {"type": "string"}},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "code", "severity", "message", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "exceptions_list",
            "description": "List exceptions (open_only default true), or get one when exception_id is set",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "exception_id": {"type": "string"},
                    "open_only": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "exceptions_set_open",
            "description": "Set exception open flag (open:false closes). No journal write.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "exception_id": {"type": "string"},
                    "open": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "exception_id", "actor_kind", "actor_id"]
            }
        }),
    ]
}
