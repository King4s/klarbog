//! Read-side queries: account balances and recent posted entries (SSR ledger).

use anyhow::Context;
use klarbog_journal::PostedEntry;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::open::{CompanyStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountBalance {
    pub account: String,
    pub debit_minor: i64,
    pub credit_minor: i64,
}

impl AccountBalance {
    /// Debit-positive net (asset/expense convention; negative = credit balance).
    pub fn net_minor(&self) -> i64 {
        self.debit_minor - self.credit_minor
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyBalance {
    pub party_id: String,
    pub debit_minor: i64,
    pub credit_minor: i64,
}

impl PartyBalance {
    /// Debit-positive net (receivable convention; negative = we owe/received).
    pub fn net_minor(&self) -> i64 {
        self.debit_minor - self.credit_minor
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostedLegView {
    pub account: String,
    pub direction: String,
    pub amount_minor: i64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostedEntryView {
    pub id: String,
    pub as_of: String,
    pub memo: String,
    pub actor: String,
    pub legs: Vec<PostedLegView>,
}

impl CompanyStore {
    /// Full posted entry (from the immutable payload) by id, if present.
    pub async fn get_posted_entry(&self, id: &str) -> Result<Option<PostedEntry>, StoreError> {
        let row = sqlx::query("SELECT payload_json FROM journal_entries WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: String = r.get("payload_json");
                Ok(Some(
                    serde_json::from_str(&payload).context("parse posted entry payload")?,
                ))
            }
        }
    }

    pub async fn account_balances(&self) -> Result<Vec<AccountBalance>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT account,
                   SUM(CASE WHEN direction = 'debit' THEN amount_minor ELSE 0 END) AS debit_minor,
                   SUM(CASE WHEN direction = 'credit' THEN amount_minor ELSE 0 END) AS credit_minor
            FROM journal_legs
            GROUP BY account
            ORDER BY account ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| AccountBalance {
                account: r.get("account"),
                debit_minor: r.get("debit_minor"),
                credit_minor: r.get("credit_minor"),
            })
            .collect())
    }

    /// Per-party debit/credit sums over legs that carry a party_id.
    pub async fn party_balances(&self) -> Result<Vec<PartyBalance>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT party_id,
                   SUM(CASE WHEN direction = 'debit' THEN amount_minor ELSE 0 END) AS debit_minor,
                   SUM(CASE WHEN direction = 'credit' THEN amount_minor ELSE 0 END) AS credit_minor
            FROM journal_legs
            WHERE party_id IS NOT NULL
            GROUP BY party_id
            ORDER BY party_id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| PartyBalance {
                party_id: r.get("party_id"),
                debit_minor: r.get("debit_minor"),
                credit_minor: r.get("credit_minor"),
            })
            .collect())
    }

    /// Latest posted entries (newest first) with their legs.
    pub async fn recent_entries(&self, limit: i64) -> Result<Vec<PostedEntryView>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT e.rowid AS erow, e.id, e.as_of, e.memo, e.actor,
                   l.account, l.direction, l.amount_minor, l.currency
            FROM journal_entries e
            JOIN journal_legs l ON l.entry_id = e.id
            WHERE e.rowid IN (
                SELECT rowid FROM journal_entries ORDER BY rowid DESC LIMIT ?1
            )
            ORDER BY e.rowid DESC, l.rowid ASC
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        let mut out: Vec<PostedEntryView> = Vec::new();
        for r in rows {
            let id: String = r.get("id");
            if out.last().map(|e| e.id != id).unwrap_or(true) {
                out.push(PostedEntryView {
                    id,
                    as_of: r.get("as_of"),
                    memo: r.get("memo"),
                    actor: r.get("actor"),
                    legs: Vec::new(),
                });
            }
            out.last_mut()
                .expect("entry pushed above")
                .legs
                .push(PostedLegView {
                    account: r.get("account"),
                    direction: r.get("direction"),
                    amount_minor: r.get("amount_minor"),
                    currency: r.get("currency"),
                });
        }
        Ok(out)
    }
}
