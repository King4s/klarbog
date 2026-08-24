//! MCP tools/list schema registry.

mod bank;
mod documents;
mod ledger_ops;

use serde_json::{json, Value};

pub fn tools_list() -> Value {
    let mut tools = Vec::new();
    tools.extend(core_tools());
    tools.extend(documents::tools());
    tools.extend(bank::tools());
    tools.extend(ledger_ops::tools());
    json!({
        "jsonrpc": "2.0",
        "result": {
            "tools": tools
        }
    })
}

fn core_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "klarbog_health",
            "description": "Health check (read-only)",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "klarbog_status",
            "description": "Status / allowlist / plugin roster (mirrors GET /api/v1/status; bind=stdio)",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "crm_upsert_party",
            "description": "Upsert CRM party (display_name; optional party_id, kind, payment_terms_days). No journal write (ADR-004).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "display_name": {"type": "string"},
                    "party_id": {"type": "string"},
                    "kind": {"type": "string", "enum": ["private", "business"]},
                    "payment_terms_days": {"type": "integer", "minimum": 1, "maximum": 365},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "display_name", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "crm_list_parties",
            "description": "List CRM parties, or get one when party_id is set (read-only)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "party_id": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
    ]
}
