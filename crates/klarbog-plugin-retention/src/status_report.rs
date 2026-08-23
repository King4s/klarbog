//! Retention status report per as-of date (original `buildRetentionStatusReport`).

use chrono::NaiveDate;
use klarbog_plugin_bank::list_bank_transactions;
use klarbog_plugin_documents::list_documents;
use klarbog_store_sqlite::open_company;
use klarbog_types::{effective_retain_until, RETENTION_RULE_ID};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::retention::RetentionError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionStatusTable {
    Documents,
    JournalEntries,
    BankTransactions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionStatusRow {
    pub table: RetentionStatusTable,
    pub total: usize,
    pub expired: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_expiry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_expired: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionStatusReport {
    pub ok: bool,
    pub as_of: String,
    pub applied_rules: Vec<String>,
    pub rows: Vec<RetentionStatusRow>,
    pub errors: Vec<String>,
}

fn summarize_rows(
    table: RetentionStatusTable,
    pairs: &[(Option<String>, Option<String>)],
    as_of: NaiveDate,
) -> Result<RetentionStatusRow, RetentionError> {
    let mut effective: Vec<NaiveDate> = Vec::new();
    for (stored, basis) in pairs {
        if let Some(date) = effective_retain_until(stored.as_deref(), basis.as_deref())
            .map_err(|e| RetentionError::Deadline(e.to_string()))?
        {
            effective.push(date);
        }
    }
    effective.sort();
    let future: Vec<_> = effective.iter().filter(|d| **d >= as_of).copied().collect();
    let expired: Vec<_> = effective.iter().filter(|d| **d < as_of).copied().collect();
    Ok(RetentionStatusRow {
        table,
        total: pairs.len(),
        expired: expired.len(),
        next_expiry: future.first().map(|d| d.format("%Y-%m-%d").to_string()),
        oldest_expired: expired.first().map(|d| d.format("%Y-%m-%d").to_string()),
    })
}

fn journal_basis_date(as_of_rfc3339: &str) -> Option<String> {
    as_of_rfc3339
        .get(0..10)
        .filter(|s| s.len() == 10)
        .map(str::to_string)
}

pub async fn build_retention_status_report(
    company: &Path,
    as_of: NaiveDate,
) -> Result<RetentionStatusReport, RetentionError> {
    let as_of_text = as_of.format("%Y-%m-%d").to_string();

    let doc_pairs: Vec<_> = list_documents(company)
        .map_err(|e| RetentionError::Deadline(e.to_string()))?
        .into_iter()
        .map(|d| {
            let basis = chrono::DateTime::from_timestamp_millis(d.created_unix_ms)
                .map(|dt| dt.date_naive().format("%Y-%m-%d").to_string());
            (d.retain_until, basis)
        })
        .collect();

    let store = open_company(company)
        .await
        .map_err(|e| RetentionError::Deadline(e.to_string()))?;
    let journal_pairs: Vec<_> = store
        .list_journal_retention_rows()
        .await
        .map_err(|e| RetentionError::Deadline(e.to_string()))?
        .into_iter()
        .map(|row| (row.retain_until, journal_basis_date(&row.as_of)))
        .collect();

    let bank_pairs: Vec<_> = list_bank_transactions(company)
        .map_err(|e| RetentionError::Deadline(e.to_string()))?
        .into_iter()
        .map(|t| (t.retain_until, Some(t.transaction_date)))
        .collect();

    let rows = vec![
        summarize_rows(RetentionStatusTable::Documents, &doc_pairs, as_of)?,
        summarize_rows(RetentionStatusTable::JournalEntries, &journal_pairs, as_of)?,
        summarize_rows(RetentionStatusTable::BankTransactions, &bank_pairs, as_of)?,
    ];

    Ok(RetentionStatusReport {
        ok: true,
        as_of: as_of_text,
        applied_rules: vec![RETENTION_RULE_ID.to_string()],
        rows,
        errors: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use klarbog_core::{init_company, Company};
    use klarbog_journal::{Direction, JournalEntry, Leg};
    use klarbog_plugin_bank::{commit_import, BankRow};
    use klarbog_plugin_documents::{attach_document, DocumentKind};
    use klarbog_types::{Actor, Currency, MinorAmount};
    use tempfile::tempdir;

    async fn smoke_company() -> (tempfile::TempDir, Company) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("co");
        let handle = init_company(&path, "Retention Test", &Actor::user("test"))
            .await
            .unwrap();
        (dir, handle)
    }

    #[tokio::test]
    async fn report_counts_objects_and_expiry_after_deadline() {
        let (_dir, company) = smoke_company().await;
        attach_document(
            &company.path,
            DocumentKind::Receipt,
            "receipts/r.pdf".into(),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let amount = MinorAmount::from_minor(10_000);
        let currency = Currency::new("DKK").unwrap();
        company
            .post(JournalEntry {
                as_of: Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0).unwrap(),
                memo: "retention smoke".into(),
                actor: Actor::user("test"),
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
            })
            .await
            .unwrap();

        let row = BankRow {
            date: Utc.with_ymd_and_hms(2026, 5, 20, 0, 0, 0).unwrap(),
            text: "Pay".into(),
            amount_minor: MinorAmount::from_minor(5_000),
        };
        commit_import(
            &company.path,
            b"csv",
            std::slice::from_ref(&row),
            Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0).unwrap(),
        )
        .unwrap();

        let before = build_retention_status_report(
            &company.path,
            NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
        )
        .await
        .unwrap();
        assert!(before.ok);
        assert_eq!(before.rows.len(), 3);
        assert_eq!(before.rows[0].total, 1);
        assert_eq!(before.rows[0].expired, 0);
        assert_eq!(before.rows[1].total, 1);
        assert_eq!(before.rows[2].total, 1);

        let after = build_retention_status_report(
            &company.path,
            NaiveDate::from_ymd_opt(2032, 1, 1).unwrap(),
        )
        .await
        .unwrap();
        assert!(after.rows.iter().all(|r| r.expired >= 1));
        assert_eq!(after.rows[0].oldest_expired.as_deref(), Some("2031-12-31"));
    }
}
