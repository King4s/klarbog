use anyhow::Context;
use klarbog_journal::{Direction, PostedEntry};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::open::{CompanyStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalDigestSummary {
    pub id: String,
    pub digest: String,
    pub prev_digest: Option<String>,
}

impl CompanyStore {
    pub async fn append(&self, posted: &PostedEntry) -> Result<(), StoreError> {
        // Re-validate at store boundary (ADR-004): empty/single/unbalanced never persist.
        posted.entry.validate()?;
        let payload = serde_json::to_string(posted).context("serialize posted entry")?;
        let mut tx = self.pool.begin().await?;
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
        .bind(&payload)
        .bind(&posted.digest)
        .bind(&posted.prev_digest)
        .execute(&mut *tx)
        .await?;
        for leg in &posted.entry.legs {
            let dir = match leg.direction {
                Direction::Debit => "debit",
                Direction::Credit => "credit",
            };
            let party = leg.party_id.as_ref().map(|p| p.as_str().to_string());
            sqlx::query(
                r#"
                INSERT INTO journal_legs(
                    entry_id, account, direction, amount_minor, currency, party_id
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
            )
            .bind(posted.id.to_string())
            .bind(&leg.account)
            .bind(dir)
            .bind(leg.amount.minor())
            .bind(leg.currency.as_str())
            .bind(party)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn last_digest(&self) -> Result<Option<String>, StoreError> {
        let row = sqlx::query("SELECT digest FROM journal_entries ORDER BY rowid DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("digest")))
    }

    pub async fn list_journal_digests(&self) -> Result<Vec<JournalDigestSummary>, StoreError> {
        let rows =
            sqlx::query("SELECT id, digest, prev_digest FROM journal_entries ORDER BY rowid ASC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|r| JournalDigestSummary {
                id: r.get("id"),
                digest: r.get("digest"),
                prev_digest: r.get("prev_digest"),
            })
            .collect())
    }

    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        let row = sqlx::query("SELECT MAX(version) AS v FROM schema_version")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<Option<i64>, _>("v").unwrap_or(0))
    }

    pub async fn pragmas(&self) -> Result<(String, i64, i64), StoreError> {
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&self.pool)
            .await?;
        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&self.pool)
            .await?;
        let busy: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&self.pool)
            .await?;
        Ok((journal, fk, busy))
    }
}
