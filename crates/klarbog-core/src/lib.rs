//! Company lifecycle and posting orchestration.

mod confirm;
mod journal_ops;
mod pathguard;
mod registry;

pub use confirm::{ConfirmStore, ConfirmToken};
pub use journal_ops::{
    journal_commit, journal_preview, payload_digest, CommitResult, PreviewResult,
};
pub use pathguard::{assert_company_path, assert_relative_path_hint, PathGuardError};
pub use registry::default_registry;

use anyhow::Context;
use klarbog_journal::{JournalEntry, PostedEntry};
use klarbog_plugin_retention::ensure_company_extras;
use klarbog_store_sqlite::{open_company, CompanyStore, StoreError};
pub use klarbog_store_sqlite::{
    AccountBalance, BankMemoRef, PartyBalance, PostedEntryView, PostedLegView,
};
use klarbog_types::Actor;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    Path(#[from] PathGuardError),
    #[error(transparent)]
    Journal(#[from] klarbog_journal::JournalError),
    #[error(transparent)]
    Confirm(#[from] klarbog_types::KlarbogError),
    #[error("rules validation failed: {0}")]
    RulesViolation(String),
    #[error("actor not in policy: {0}")]
    ActorDenied(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyPolicy {
    pub name: String,
    pub actors: Vec<String>,
}

pub struct Company {
    pub path: PathBuf,
    pub policy: CompanyPolicy,
    store: CompanyStore,
}

impl std::fmt::Debug for Company {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Company")
            .field("path", &self.path)
            .field("policy", &self.policy)
            .finish()
    }
}

pub async fn init_company(path: &Path, name: &str, owner: &Actor) -> Result<Company, CoreError> {
    std::fs::create_dir_all(path).context("mkdir")?;
    let policy = CompanyPolicy {
        name: name.to_string(),
        actors: vec![owner.as_tag()],
    };
    let policy_path = path.join("policy.json");
    std::fs::write(
        &policy_path,
        serde_json::to_string_pretty(&policy).context("policy json")?,
    )
    .context("write policy")?;
    ensure_company_extras(path).map_err(|e| CoreError::Other(anyhow::anyhow!(e)))?;
    let store = open_company(path).await?;
    Ok(Company {
        path: path.to_path_buf(),
        policy,
        store,
    })
}

pub async fn open_existing(path: &Path) -> Result<Company, CoreError> {
    let policy: CompanyPolicy = serde_json::from_str(
        &std::fs::read_to_string(path.join("policy.json")).context("read policy")?,
    )
    .context("parse policy")?;
    let store = open_company(path).await?;
    Ok(Company {
        path: path.to_path_buf(),
        policy,
        store,
    })
}

impl Company {
    pub fn authorize(&self, actor: &Actor) -> Result<(), CoreError> {
        let tag = actor.as_tag();
        if self.policy.actors.iter().any(|a| a == &tag) {
            Ok(())
        } else {
            Err(CoreError::ActorDenied(tag))
        }
    }

    pub async fn post(&self, entry: JournalEntry) -> Result<PostedEntry, CoreError> {
        self.authorize(&entry.actor)?;
        let prev = self.store.last_digest().await?;
        let posted = entry.post(prev).map_err(StoreError::from)?;
        self.store.append(&posted).await?;
        Ok(posted)
    }

    pub async fn account_balances(&self) -> Result<Vec<AccountBalance>, CoreError> {
        Ok(self.store.account_balances().await?)
    }

    pub async fn recent_entries(&self, limit: i64) -> Result<Vec<PostedEntryView>, CoreError> {
        Ok(self.store.recent_entries(limit).await?)
    }

    pub async fn posted_entry(&self, id: &str) -> Result<Option<PostedEntry>, CoreError> {
        Ok(self.store.get_posted_entry(id).await?)
    }

    pub async fn party_balances(&self) -> Result<Vec<PartyBalance>, CoreError> {
        Ok(self.store.party_balances().await?)
    }

    /// Posted entries with traceable `bank:...` memos (reconciliation report).
    pub async fn bank_posted_refs(&self) -> Result<Vec<BankMemoRef>, CoreError> {
        Ok(self.store.bank_posted_refs().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_journal::{Direction, JournalEntry, Leg};
    use klarbog_types::{Currency, MinorAmount};
    use tempfile::tempdir;

    fn expense(actor: Actor, minor: i64) -> JournalEntry {
        let amount = MinorAmount::from_minor(minor);
        let currency = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: "core test".into(),
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
    async fn init_posts_for_policy_actor() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = init_company(dir.path(), "Demo ApS", &owner).await.unwrap();
        assert!(dir.path().join("retention.json").exists());
        assert!(dir.path().join("templates/expense_memo.md").exists());
        let posted = company.post(expense(owner, 250)).await.unwrap();
        assert!(!posted.digest.is_empty());
    }

    #[tokio::test]
    async fn balances_and_recent_entries_reflect_posts() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = init_company(dir.path(), "Demo ApS", &owner).await.unwrap();
        company.post(expense(owner.clone(), 250)).await.unwrap();
        company.post(expense(owner, 100)).await.unwrap();

        let balances = company.account_balances().await.unwrap();
        let b3000 = balances.iter().find(|b| b.account == "3000").unwrap();
        assert_eq!((b3000.debit_minor, b3000.credit_minor), (350, 0));
        assert_eq!(b3000.net_minor(), 350);
        let b2000 = balances.iter().find(|b| b.account == "2000").unwrap();
        assert_eq!(b2000.net_minor(), -350);

        let recent = company.recent_entries(10).await.unwrap();
        assert_eq!(recent.len(), 2);
        // Newest first, each with both legs.
        assert_eq!(recent[0].legs.len(), 2);
        assert_eq!(recent[0].legs[0].amount_minor, 100);
        assert_eq!(recent[1].legs[0].amount_minor, 250);

        let limited = company.recent_entries(1).await.unwrap();
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].legs[0].amount_minor, 100);
    }

    #[tokio::test]
    async fn party_balances_group_party_legs() {
        use klarbog_types::PartyId;
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = init_company(dir.path(), "Demo ApS", &owner).await.unwrap();
        let mut entry = expense(owner, 500);
        entry.legs[0].party_id = Some(PartyId::new("party_a"));
        company.post(entry).await.unwrap();

        let balances = company.party_balances().await.unwrap();
        assert_eq!(balances.len(), 1);
        assert_eq!(balances[0].party_id, "party_a");
        assert_eq!(balances[0].net_minor(), 500);
    }

    #[tokio::test]
    async fn unknown_actor_denied() {
        let dir = tempdir().unwrap();
        let company = init_company(dir.path(), "Demo ApS", &Actor::user("owner"))
            .await
            .unwrap();
        let err = company
            .post(expense(Actor::user("intruder"), 10))
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::ActorDenied(_)));
    }
}
