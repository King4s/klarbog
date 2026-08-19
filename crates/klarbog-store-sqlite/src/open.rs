use anyhow::Context;
use klarbog_journal::{JournalError, PostedEntry};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct CompanyStore {
    pool: SqlitePool,
}

pub async fn open_company(dir: &Path) -> Result<CompanyStore, StoreError> {
    std::fs::create_dir_all(dir).context("create company dir")?;
    let db = dir.join("ledger.sqlite");
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}?mode=rwc", db.display()))?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(1) // single-writer serialization
        .connect_with(opts)
        .await?;
    let store = CompanyStore { pool };
    store.migrate().await?;
    Ok(store)
}

impl CompanyStore {
    async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
              version INTEGER NOT NULL PRIMARY KEY
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        let row = sqlx::query("SELECT MAX(version) as v FROM schema_version")
            .fetch_one(&self.pool)
            .await?;
        let current: Option<i64> = row.try_get("v")?;
        if current.unwrap_or(0) < 1 {
            sqlx::query(
                r#"
                CREATE TABLE journal_entries (
                  id TEXT PRIMARY KEY NOT NULL,
                  as_of TEXT NOT NULL,
                  memo TEXT NOT NULL,
                  actor TEXT NOT NULL,
                  payload_json TEXT NOT NULL,
                  digest TEXT NOT NULL,
                  prev_digest TEXT
                );
                "#,
            )
            .execute(&self.pool)
            .await?;
            sqlx::query("INSERT INTO schema_version(version) VALUES (1)")
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn append(&self, posted: &PostedEntry) -> Result<(), StoreError> {
        // Re-validate at store boundary (ADR-004)
        posted.entry.validate()?;
        let payload = serde_json::to_string(posted).context("serialize")?;
        sqlx::query(
            r#"
            INSERT INTO journal_entries(id, as_of, memo, actor, payload_json, digest, prev_digest)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(posted.id.to_string())
        .bind(posted.entry.as_of.to_rfc3339())
        .bind(&posted.entry.memo)
        .bind(posted.entry.actor.as_tag())
        .bind(payload)
        .bind(&posted.digest)
        .bind(&posted.prev_digest)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn last_digest(&self) -> Result<Option<String>, StoreError> {
        let row = sqlx::query("SELECT digest FROM journal_entries ORDER BY rowid DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("digest")))
    }

    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        let row = sqlx::query("SELECT MAX(version) as v FROM schema_version")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<Option<i64>, _>("v").unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_journal::{Direction, JournalEntry, Leg};
    use klarbog_types::{Actor, Currency, MinorAmount};
    use tempfile::tempdir;

    #[tokio::test]
    async fn wal_and_balanced_append() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        assert_eq!(store.schema_version().await.unwrap(), 1);
        let (a, c) = (MinorAmount::from_minor(100), Currency::new("DKK").unwrap());
        let entry = JournalEntry {
            as_of: Utc::now(),
            memo: "t".into(),
            actor: Actor::user("u"),
            legs: vec![
                Leg {
                    account: "6000".into(),
                    direction: Direction::Debit,
                    amount: a,
                    currency: c.clone(),
                    party_id: None,
                },
                Leg {
                    account: "5800".into(),
                    direction: Direction::Credit,
                    amount: a,
                    currency: c,
                    party_id: None,
                },
            ],
        };
        let posted = entry.post(None).unwrap();
        store.append(&posted).await.unwrap();
        assert_eq!(
            store.last_digest().await.unwrap().as_deref(),
            Some(posted.digest.as_str())
        );
    }
}
