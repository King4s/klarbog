//! Read-side query for the bank reconciliation period report: posted
//! entries whose memo follows the traceable `bank:...` convention.

use sqlx::Row;

use crate::open::{CompanyStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankMemoRef {
    /// RFC 3339 timestamp as stored on the entry.
    pub as_of: String,
    pub memo: String,
}

impl CompanyStore {
    pub async fn bank_posted_refs(&self) -> Result<Vec<BankMemoRef>, StoreError> {
        let rows = sqlx::query(
            "SELECT as_of, memo FROM journal_entries WHERE memo LIKE 'bank:%' ORDER BY as_of ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| BankMemoRef {
                as_of: r.get("as_of"),
                memo: r.get("memo"),
            })
            .collect())
    }
}
