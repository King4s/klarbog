//! MCP tool handlers (slice 2+3).

use klarbog_core::{journal_commit, journal_preview, ConfirmStore, CoreError};
use klarbog_journal::JournalEntry;
use klarbog_plugin::Registry;
use klarbog_types::{Actor, ActorKind, Envelope, KlarbogError};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn default_allowlist_root() -> PathBuf {
    std::env::var("KLARBOG_ALLOWLIST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/pellucid-software/klarbog"))
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

fn parse_actor(args: &Value) -> Result<Actor, String> {
    let kind_raw = args
        .get("actor_kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing actor_kind".to_string())?;
    let id = args
        .get("actor_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing actor_id".to_string())?;
    let kind = match kind_raw {
        "user" => ActorKind::User,
        "agent" => ActorKind::Agent,
        "system" => ActorKind::System,
        other => return Err(format!("invalid actor_kind: {other}")),
    };
    Ok(Actor {
        kind,
        id: id.to_string(),
    })
}

fn parse_entry(args: &Value) -> Result<JournalEntry, String> {
    serde_json::from_value(
        args.get("entry")
            .cloned()
            .ok_or_else(|| "missing entry".to_string())?,
    )
    .map_err(|e| format!("invalid entry: {e}"))
}

fn parse_company(args: &Value) -> Result<PathBuf, String> {
    args.get("company")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "missing company".to_string())
}

fn map_core_error(err: CoreError) -> Envelope<Value> {
    match err {
        CoreError::ActorDenied(tag) => Envelope::err([format!("actor not in policy: {tag}")]),
        CoreError::Path(e) => Envelope::err([e.to_string()]),
        CoreError::Journal(e) => Envelope::err([e.to_string()]),
        CoreError::RulesViolation(msg) => Envelope::err([msg]),
        CoreError::Confirm(KlarbogError::ConfirmRequired) => {
            Envelope::err(["confirm token required or already consumed"])
        }
        CoreError::Confirm(e) => Envelope::err([e.to_string()]),
        CoreError::Store(e) => Envelope::err([e.to_string()]),
        CoreError::Other(e) => Envelope::err([e.to_string()]),
    }
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
        "journal_post_preview" => {
            let actor = match parse_actor(args) {
                Ok(a) => a,
                Err(e) => return Envelope::err([e]),
            };
            let company = match parse_company(args) {
                Ok(c) => c,
                Err(e) => return Envelope::err([e]),
            };
            let entry = match parse_entry(args) {
                Ok(e) => e,
                Err(e) => return Envelope::err([e]),
            };
            match rt.block_on(journal_preview(
                allowlist_root,
                &company,
                &entry,
                &actor,
                store,
                registry,
            )) {
                Ok(result) => Envelope::ok_with_rules(
                    json!({
                        "confirm_token": result.confirm_token.token,
                        "expires_unix_ms": result.confirm_token.expires_unix_ms,
                        "payload_digest": result.payload_digest,
                    }),
                    result.applied_rules,
                ),
                Err(e) => map_core_error(e),
            }
        }
        "journal_post_commit" => {
            let actor = match parse_actor(args) {
                Ok(a) => a,
                Err(e) => return Envelope::err([e]),
            };
            let company = match parse_company(args) {
                Ok(c) => c,
                Err(e) => return Envelope::err([e]),
            };
            let entry = match parse_entry(args) {
                Ok(e) => e,
                Err(e) => return Envelope::err([e]),
            };
            let token = match args.get("confirm_token").and_then(|v| v.as_str()) {
                Some(t) if !t.is_empty() => t,
                _ => return Envelope::err(["missing confirm_token"]),
            };
            match rt.block_on(journal_commit(
                allowlist_root,
                &company,
                entry,
                &actor,
                token,
                store,
                registry,
            )) {
                Ok(result) => Envelope::ok_with_rules(
                    json!({
                        "id": result.posted.id,
                        "digest": result.posted.digest,
                        "prev_digest": result.posted.prev_digest,
                    }),
                    result.applied_rules,
                ),
                Err(e) => map_core_error(e),
            }
        }
        other => Envelope::err([format!("unknown tool: {other}")]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_core::default_registry;
    use klarbog_core::init_company;
    use klarbog_journal::{Direction, Leg};
    use klarbog_types::{Currency, MinorAmount};
    use tempfile::tempdir;

    fn sample_entry(actor: Actor, minor: i64) -> JournalEntry {
        let amount = MinorAmount::from_minor(minor);
        let currency = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: "mcp test".into(),
            actor: actor.clone(),
            legs: vec![
                Leg {
                    account: "6000".into(),
                    direction: Direction::Debit,
                    amount,
                    currency: currency.clone(),
                    party_id: None,
                },
                Leg {
                    account: "5800".into(),
                    direction: Direction::Credit,
                    amount,
                    currency,
                    party_id: None,
                },
            ],
        }
    }

    #[test]
    fn tools_list_names() {
        let listed = tools_list();
        let tools = listed["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"klarbog_health"));
        assert!(names.contains(&"journal_post_preview"));
        assert!(names.contains(&"journal_post_commit"));
    }

    #[test]
    fn preview_commit_via_handler() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        rt.block_on(init_company(&company_path, "Demo", &owner))
            .unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let entry = sample_entry(owner.clone(), 200);
        let args = json!({
            "company": company_path.to_string_lossy(),
            "entry": entry,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let preview = handle_tool_call(
            "journal_post_preview",
            &args,
            &store,
            dir.path(),
            &registry,
            &rt,
        );
        assert!(preview.ok);
        assert!(preview
            .applied_rules
            .contains(&"dk.expense.hint".to_string()));
        let token = preview.data.unwrap()["confirm_token"]
            .as_str()
            .unwrap()
            .to_string();
        let entry2 = entry.clone();
        let commit_args = json!({
            "company": company_path.to_string_lossy(),
            "entry": entry2,
            "confirm_token": token,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let commit = handle_tool_call(
            "journal_post_commit",
            &commit_args,
            &store,
            dir.path(),
            &registry,
            &rt,
        );
        assert!(commit.ok);
    }
}
