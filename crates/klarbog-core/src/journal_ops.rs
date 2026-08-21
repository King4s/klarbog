//! Shared journal preview/commit for API and MCP (slice 2+3).

use crate::{
    assert_company_path, open_existing, ConfirmStore, ConfirmToken, CoreError, PathGuardError,
};
use klarbog_journal::{JournalEntry, PostedEntry};
use klarbog_plugin::Registry;
use klarbog_types::Actor;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PreviewResult {
    pub confirm_token: ConfirmToken,
    pub payload_digest: String,
    pub applied_rules: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommitResult {
    pub posted: PostedEntry,
    pub applied_rules: Vec<String>,
}

/// Stable digest of company path + canonical JSON entry (two-phase bind).
pub fn payload_digest(company_path: &Path, entry: &JournalEntry) -> Result<String, CoreError> {
    let entry_json = serde_json::to_string(entry).map_err(|e| CoreError::Other(e.into()))?;
    let mut h = Sha256::new();
    h.update(company_path.to_string_lossy().as_bytes());
    h.update(b"\0");
    h.update(entry_json.as_bytes());
    Ok(hex::encode(h.finalize()))
}

pub fn resolve_company_path(
    allowlist_root: &Path,
    company: &Path,
) -> Result<PathBuf, PathGuardError> {
    assert_company_path(allowlist_root, company)
}

/// Bind request actor onto the entry so AuthZ cannot be bypassed via body spoof.
fn bind_actor(mut entry: JournalEntry, actor: &Actor) -> JournalEntry {
    entry.actor = actor.clone();
    entry
}

fn run_rules(registry: &Registry, entry: &JournalEntry) -> Result<Vec<String>, CoreError> {
    registry
        .validate_rules(entry)
        .map_err(|e| CoreError::RulesViolation(e.to_string()))
}

/// Phase 1: validate, authorize, issue token — no write.
pub async fn journal_preview(
    allowlist_root: &Path,
    company: &Path,
    entry: &JournalEntry,
    actor: &Actor,
    store: &ConfirmStore,
    registry: &Registry,
) -> Result<PreviewResult, CoreError> {
    let entry = bind_actor(entry.clone(), actor);
    entry.validate()?;
    let applied_rules = run_rules(registry, &entry)?;
    let path = resolve_company_path(allowlist_root, company)?;
    let company_obj = open_existing(&path).await?;
    company_obj.authorize(actor)?;
    let digest = payload_digest(&path, &entry)?;
    let confirm_token = store.issue(&digest);
    Ok(PreviewResult {
        confirm_token,
        payload_digest: digest,
        applied_rules,
    })
}

/// Phase 2: consume token, authorize, post.
pub async fn journal_commit(
    allowlist_root: &Path,
    company: &Path,
    entry: JournalEntry,
    actor: &Actor,
    confirm_token: &str,
    store: &ConfirmStore,
    registry: &Registry,
) -> Result<CommitResult, CoreError> {
    let entry = bind_actor(entry, actor);
    entry.validate()?;
    let applied_rules = run_rules(registry, &entry)?;
    let path = resolve_company_path(allowlist_root, company)?;
    let digest = payload_digest(&path, &entry)?;
    store.consume(confirm_token, &digest)?;
    let company_obj = open_existing(&path).await?;
    company_obj.authorize(actor)?;
    let posted = company_obj.post(entry).await?;
    Ok(CommitResult {
        posted,
        applied_rules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_journal::{Direction, Leg};
    use klarbog_types::{Currency, MinorAmount};
    use tempfile::tempdir;

    use crate::{default_registry, init_company};

    fn balanced_entry(actor: Actor, minor: i64, memo: &str) -> JournalEntry {
        let amount = MinorAmount::from_minor(minor);
        let currency = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: memo.into(),
            actor,
            legs: vec![
                Leg {
                    account: "3000".into(),
                    direction: Direction::Debit,
                    amount,
                    currency: currency.clone(),
                    party_id: None,
                },
                Leg {
                    account: "2000".into(),
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
        let root = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = root.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let entry = balanced_entry(owner.clone(), 500, "ops test #receipt");
        let preview = journal_preview(
            root.path(),
            &company_path,
            &entry,
            &owner,
            &store,
            &registry,
        )
        .await
        .unwrap();
        assert!(!preview
            .applied_rules
            .contains(&"dk.expense.receipt_hint".to_string()));
        let result = journal_commit(
            root.path(),
            &company_path,
            entry,
            &owner,
            &preview.confirm_token.token,
            &store,
            &registry,
        )
        .await
        .unwrap();
        assert!(!result.posted.digest.is_empty());
        assert!(!result
            .applied_rules
            .contains(&"dk.expense.receipt_hint".to_string()));
    }

    #[tokio::test]
    async fn rules_block_expense_without_receipt() {
        let root = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = root.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let entry = balanced_entry(owner.clone(), 100, "ops test");
        let err = journal_preview(
            root.path(),
            &company_path,
            &entry,
            &owner,
            &store,
            &registry,
        )
        .await
        .unwrap_err();
        match err {
            CoreError::RulesViolation(msg) => {
                assert!(msg.contains("dk.expense.receipt_required"));
            }
            other => panic!("expected RulesViolation, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rules_reject_empty_memo() {
        let root = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = root.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let entry = balanced_entry(owner.clone(), 100, "   ");
        let err = journal_preview(
            root.path(),
            &company_path,
            &entry,
            &owner,
            &store,
            &registry,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::RulesViolation(_)));
    }

    #[tokio::test]
    async fn rules_reject_zero_amount() {
        let root = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = root.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let entry = balanced_entry(owner.clone(), 0, "zero leg");
        let err = journal_preview(
            root.path(),
            &company_path,
            &entry,
            &owner,
            &store,
            &registry,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, CoreError::RulesViolation(_)));
    }

    #[tokio::test]
    async fn wrong_token_fails() {
        let root = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = root.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let store = ConfirmStore::default();
        let registry = default_registry();
        let entry = balanced_entry(owner.clone(), 100, "ops test #receipt");
        journal_preview(
            root.path(),
            &company_path,
            &entry,
            &owner,
            &store,
            &registry,
        )
        .await
        .unwrap();
        let err = journal_commit(
            root.path(),
            &company_path,
            entry,
            &owner,
            "bad-token",
            &store,
            &registry,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            CoreError::Confirm(klarbog_types::KlarbogError::ConfirmRequired)
        ));
    }

    #[tokio::test]
    async fn path_outside_allowlist_fails() {
        let root = tempdir().unwrap();
        let outside = PathBuf::from("/tmp/klarbog-outside-test");
        let err = resolve_company_path(root.path(), &outside).unwrap_err();
        assert_eq!(err, PathGuardError::OutsideAllowlist);
    }
}
