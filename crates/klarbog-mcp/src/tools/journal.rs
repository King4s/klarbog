//! Journal two-phase confirm MCP tools.

use super::auth::{map_core_error, parse_actor, parse_company};
use klarbog_core::{journal_commit, journal_preview, ConfirmStore};
use klarbog_journal::JournalEntry;
use klarbog_plugin::Registry;
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

fn parse_entry(args: &Value) -> Result<JournalEntry, String> {
    serde_json::from_value(
        args.get("entry")
            .cloned()
            .ok_or_else(|| "missing entry".to_string())?,
    )
    .map_err(|e| format!("invalid entry: {e}"))
}

pub async fn journal_post_preview(
    args: &Value,
    allowlist_root: &Path,
    store: &ConfirmStore,
    registry: &Registry,
) -> Envelope<Value> {
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
    match journal_preview(allowlist_root, &company, &entry, &actor, store, registry).await {
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

pub async fn journal_post_commit(
    args: &Value,
    allowlist_root: &Path,
    store: &ConfirmStore,
    registry: &Registry,
) -> Envelope<Value> {
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
    match journal_commit(
        allowlist_root,
        &company,
        entry,
        &actor,
        token,
        store,
        registry,
    )
    .await
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_core::{default_registry, init_company};
    use klarbog_journal::{Direction, Leg};
    use klarbog_types::{Actor, Currency, MinorAmount};
    use tempfile::tempdir;

    fn sample_entry(actor: Actor, minor: i64) -> JournalEntry {
        let amount = MinorAmount::from_minor(minor);
        let currency = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: "mcp test #receipt".into(),
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

    #[tokio::test]
    async fn preview_commit_roundtrip() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let entry = sample_entry(owner.clone(), 200);
        let args = json!({
            "company": company_path.to_string_lossy(),
            "entry": entry,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let preview = journal_post_preview(&args, dir.path(), &store, &registry).await;
        assert!(preview.ok);
        let token = preview.data.unwrap()["confirm_token"]
            .as_str()
            .unwrap()
            .to_string();
        let commit_args = json!({
            "company": company_path.to_string_lossy(),
            "entry": entry,
            "confirm_token": token,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let commit = journal_post_commit(&commit_args, dir.path(), &store, &registry).await;
        assert!(commit.ok);
    }
}
