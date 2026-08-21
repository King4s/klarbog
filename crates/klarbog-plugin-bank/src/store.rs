//! Per-company `bank_transactions.json` — imported rows with batch trail
//! (DK-BOOKKEEPING-BANK-IMPORT-001). File store only (ADR-006: no sqlite).

use crate::batch::{fingerprints_for_rows, make_import_batch_id, source_file_hash};
use crate::csv::BankRow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const BANK_TRANSACTIONS_FILENAME: &str = "bank_transactions.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankTransaction {
    pub id: String,
    pub transaction_date: String,
    pub text: String,
    pub amount_minor: i64,
    pub currency: String,
    pub source_file_hash: String,
    pub import_batch_id: String,
    pub transaction_hash: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BankTransactionsFile {
    transactions: Vec<BankTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportCommitResult {
    pub import_batch_id: String,
    pub source_file_hash: String,
    pub imported: usize,
    pub skipped_duplicates: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankStoreError {
    #[error("io: {0}")]
    Io(String),
    #[error("json: {0}")]
    Json(String),
    #[error("empty import")]
    Empty,
}

impl From<std::io::Error> for BankStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for BankStoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e.to_string())
    }
}

fn path(company: &Path) -> PathBuf {
    company.join(BANK_TRANSACTIONS_FILENAME)
}

fn load(company: &Path) -> Result<BankTransactionsFile, BankStoreError> {
    let p = path(company);
    if !p.exists() {
        return Ok(BankTransactionsFile::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(p)?)?)
}

fn save(company: &Path, file: &BankTransactionsFile) -> Result<(), BankStoreError> {
    fs::write(path(company), serde_json::to_string_pretty(file)?)?;
    Ok(())
}

pub fn list_bank_transactions(company: &Path) -> Result<Vec<BankTransaction>, BankStoreError> {
    Ok(load(company)?.transactions)
}

/// Persist parsed rows with batch trail. Duplicates (same fingerprint already
/// in the company) are skipped — originalens import behaviour, satisfying
/// `forbid: duplicate_transaction_fingerprint_in_same_company`.
pub fn commit_import(
    company: &Path,
    csv_bytes: &[u8],
    rows: &[BankRow],
    as_of: DateTime<Utc>,
) -> Result<ImportCommitResult, BankStoreError> {
    if rows.is_empty() {
        return Err(BankStoreError::Empty);
    }
    let source_hash = source_file_hash(csv_bytes);
    let batch_id = make_import_batch_id(as_of, &source_hash);
    let fingerprints = fingerprints_for_rows(rows);

    let mut file = load(company)?;
    let existing: HashSet<String> = file
        .transactions
        .iter()
        .map(|t| t.transaction_hash.clone())
        .collect();

    let mut imported = 0usize;
    let mut skipped = 0usize;
    for (row, hash) in rows.iter().zip(fingerprints.iter()) {
        if existing.contains(hash) {
            skipped += 1;
            continue;
        }
        file.transactions.push(BankTransaction {
            id: format!("btx_{}", Uuid::new_v4()),
            transaction_date: row.date.format("%Y-%m-%d").to_string(),
            text: row.text.trim().to_string(),
            amount_minor: row.amount_minor.minor(),
            currency: "DKK".into(),
            source_file_hash: source_hash.clone(),
            import_batch_id: batch_id.clone(),
            transaction_hash: hash.clone(),
            status: "imported".into(),
        });
        imported += 1;
    }
    save(company, &file)?;
    Ok(ImportCommitResult {
        import_batch_id: batch_id,
        source_file_hash: source_hash,
        imported,
        skipped_duplicates: skipped,
    })
}

/// Convert stored transactions to BankRow for reconciliation_report.
pub fn rows_from_transactions(txs: &[BankTransaction]) -> Result<Vec<BankRow>, BankStoreError> {
    txs.iter()
        .map(|t| {
            let date = format!("{}T00:00:00Z", t.transaction_date)
                .parse::<DateTime<Utc>>()
                .map_err(|e| BankStoreError::Io(format!("bad date {}: {e}", t.transaction_date)))?;
            Ok(BankRow {
                date,
                text: t.text.clone(),
                amount_minor: klarbog_types::MinorAmount::from_minor(t.amount_minor),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_types::MinorAmount;
    use tempfile::tempdir;

    fn row(date: &str, text: &str, minor: i64) -> BankRow {
        BankRow {
            date: format!("{date}T00:00:00Z").parse().unwrap(),
            text: text.into(),
            amount_minor: MinorAmount::from_minor(minor),
        }
    }

    #[test]
    fn commit_assigns_batch_hash_and_skips_duplicates() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let csv = b"Dato;Tekst;Belob\n2026-05-20;Pay;100,00\n";
        let rows = [row("2026-05-20", "Pay", 10_000)];
        let as_of = "2026-05-20T12:00:00Z".parse().unwrap();
        let r1 = commit_import(&co, csv, &rows, as_of).unwrap();
        assert_eq!(r1.imported, 1);
        assert_eq!(r1.skipped_duplicates, 0);
        assert!(r1.import_batch_id.starts_with("BANK-"));
        assert_eq!(r1.source_file_hash.len(), 64);

        let listed = list_bank_transactions(&co).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].import_batch_id, r1.import_batch_id);
        assert_eq!(listed[0].source_file_hash, r1.source_file_hash);
        assert!(!listed[0].transaction_hash.is_empty());

        // Re-import same fingerprint → skip, no second row.
        let r2 = commit_import(&co, csv, &rows, as_of).unwrap();
        assert_eq!(r2.imported, 0);
        assert_eq!(r2.skipped_duplicates, 1);
        assert_eq!(list_bank_transactions(&co).unwrap().len(), 1);
    }

    #[test]
    fn identical_rows_in_same_file_both_import() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let rows = [
            row("2026-05-20", "Fee", 5_000),
            row("2026-05-20", "Fee", 5_000),
        ];
        let r = commit_import(&co, b"csv", &rows, Utc::now()).unwrap();
        assert_eq!(r.imported, 2);
        assert_eq!(list_bank_transactions(&co).unwrap().len(), 2);
    }
}
