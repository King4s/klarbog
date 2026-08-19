//! MCP tool registry and dispatch.

mod auth;
mod bank;
mod invoice;
mod journal;
mod retention;

use klarbog_core::ConfirmStore;
use klarbog_plugin::Registry;
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn default_allowlist_root() -> PathBuf {
    std::env::var("KLARBOG_ALLOWLIST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn tools_list() -> Value {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "tools": [
                {
                    "name": "klarbog_health",
                    "description": "Health check (read-only)",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "bank_import_preview",
                    "description": "Import bank transactions (API or CSV fallback) and return draft journal entries (read-only, no post)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "provider": {"type": "string", "enum": ["generic_dk", "revolut", "stripe"]},
                            "source": {"type": "string", "enum": ["api", "csv"]},
                            "csv": {"type": "string"},
                            "currency": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "provider", "actor_kind", "actor_id"]
                    }
                },
                {
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
                },
                {
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
                },
                {
                    "name": "invoice_mark_paid_preview",
                    "description": "Mark invoice paid and return payment journal suggestion with party_id (no post)",
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
                },
                {
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
                },
                {
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
                }
            ]
        }
    })
}

pub fn handle_tool_call(
    name: &str,
    args: &Value,
    store: &ConfirmStore,
    allowlist_root: &Path,
    registry: &Registry,
    rt: &tokio::runtime::Runtime,
) -> Envelope<Value> {
    match name {
        "klarbog_health" => Envelope::ok(json!({
            "ok": true,
            "service": "klarbog-mcp",
            "allowlist_root": allowlist_root,
        })),
        "bank_import_preview" => rt.block_on(bank::bank_import_preview(args, allowlist_root)),
        "retention_get" => rt.block_on(retention::retention_get(args, allowlist_root)),
        "backup_manifest" => rt.block_on(retention::backup_manifest(args, allowlist_root)),
        "invoice_mark_paid_preview" => {
            rt.block_on(invoice::invoice_mark_paid_preview(args, allowlist_root))
        }
        "journal_post_preview" => rt.block_on(journal::journal_post_preview(
            args,
            allowlist_root,
            store,
            registry,
        )),
        "journal_post_commit" => rt.block_on(journal::journal_post_commit(
            args,
            allowlist_root,
            store,
            registry,
        )),
        other => Envelope::err([format!("unknown tool: {other}")]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_core::default_registry;

    #[test]
    fn tools_list_names() {
        let listed = tools_list();
        let tools = listed["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"klarbog_health"));
        assert!(names.contains(&"bank_import_preview"));
        assert!(names.contains(&"retention_get"));
        assert!(names.contains(&"backup_manifest"));
        assert!(names.contains(&"invoice_mark_paid_preview"));
        assert!(names.contains(&"journal_post_preview"));
        assert!(names.contains(&"journal_post_commit"));
    }

    #[test]
    fn health_via_handler() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let env = handle_tool_call(
            "klarbog_health",
            &json!({}),
            &store,
            Path::new("/tmp"),
            &registry,
            &rt,
        );
        assert!(env.ok);
    }
}
