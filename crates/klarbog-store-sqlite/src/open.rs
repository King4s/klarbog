use anyhow::Context;
use klarbog_journal::JournalError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct CompanyStore {
    pub(crate) pool: SqlitePool,
}

pub async fn open_company(dir: &Path) -> Result<CompanyStore, StoreError> {
    std::fs::create_dir_all(dir).context("create company dir")?;
    let db = dir.join("ledger.sqlite");
    let opts = SqliteConnectOptions::new()
        .filename(&db)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    let store = CompanyStore { pool };
    store.migrate().await?;
    Ok(store)
}

impl CompanyStore {
    async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_journal::{Direction, JournalEntry, Leg};
    use klarbog_types::{Actor, Currency, MinorAmount};
    use tempfile::tempdir;

    fn balanced(minor: i64) -> JournalEntry {
        let a = MinorAmount::from_minor(minor);
        let c = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: "t".into(),
            actor: Actor::user("u"),
            legs: vec![
                Leg {
                    account: "3000".into(),
                    direction: Direction::Debit,
                    amount: a,
                    currency: c.clone(),
                    party_id: None,
                },
                Leg {
                    account: "2000".into(),
                    direction: Direction::Credit,
                    amount: a,
                    currency: c,
                    party_id: None,
                },
            ],
        }
    }

    #[tokio::test]
    async fn wal_fk_and_balanced_append() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        assert_eq!(store.schema_version().await.unwrap(), 2);
        let (mode, fk, busy) = store.pragmas().await.unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        assert_eq!(fk, 1);
        assert!(busy >= 5000);
        let posted = balanced(100).post(None).unwrap();
        store.append(&posted).await.unwrap();
        let rows = store.list_journal_retention_rows().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].retain_until.is_some());
        assert_eq!(
            store.last_digest().await.unwrap().as_deref(),
            Some(posted.digest.as_str())
        );
    }

    #[tokio::test]
    async fn store_rejects_unbalanced() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        let mut entry = balanced(100);
        entry.legs[1].amount = MinorAmount::from_minor(1);
        let posted = {
            // bypass JournalEntry::post (which also validates) by constructing via post on
            // a clone that we force-validate skip — store must still refuse.
            let ok = balanced(40).post(None).unwrap();
            let mut bad = ok;
            bad.entry = entry;
            bad
        };
        assert!(store.append(&posted).await.is_err());
    }
}
