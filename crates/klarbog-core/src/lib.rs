//! Company lifecycle and posting orchestration.

use anyhow::Context;
use klarbog_journal::{JournalEntry, PostedEntry};
use klarbog_store_sqlite::{open_company, CompanyStore, StoreError};
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
}
