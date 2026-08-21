//! Import batch identity + deterministic fingerprints — porteret fra
//! originalens `core/bank.ts` (DK-BOOKKEEPING-BANK-IMPORT-001):
//! `import_batch_id`, `source_file_hash`, `transaction_hash`.

use crate::csv::BankRow;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

/// SHA-256 hex of the raw CSV/source bytes (originalens `sourceFileHash`).
pub fn source_file_hash(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// `BANK-{YYYYMMDDhhmmss}-{hash8}` — same shape as originalens importBatchId.
pub fn make_import_batch_id(as_of: DateTime<Utc>, source_hash: &str) -> String {
    let stamp = as_of.format("%Y%m%d%H%M%S").to_string();
    let prefix: String = source_hash.chars().take(8).collect();
    format!("BANK-{stamp}-{prefix}")
}

/// Deterministic per-row fingerprint. Port uses `amount_minor` (øre) where
/// original used DKK float — same fields otherwise (date/text/currency/
/// occurrence). `bank_account_id` is null until multi-account is ported.
pub fn transaction_fingerprint(row: &BankRow, occurrence: u32) -> String {
    let date = row.date.format("%Y-%m-%d").to_string();
    let payload = serde_json::json!({
        "bank_account_id": null,
        "transaction_date": date,
        "booking_date": null,
        "text": row.text.trim(),
        "amount_minor": row.amount_minor.minor(),
        "currency": "DKK",
        "reference": null,
        "occurrence": occurrence,
    });
    let mut h = Sha256::new();
    h.update(payload.to_string().as_bytes());
    hex::encode(h.finalize())
}

/// Content-key (occurrence=0) used to count identical rows within one file,
/// matching originalens occurrenceByContent map.
pub fn content_fingerprint(row: &BankRow) -> String {
    transaction_fingerprint(row, 0)
}

/// Assign occurrence-aware fingerprints for a batch of rows (in file order).
pub fn fingerprints_for_rows(rows: &[BankRow]) -> Vec<String> {
    use std::collections::HashMap;
    let mut occurrence_by_content: HashMap<String, u32> = HashMap::new();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let content = content_fingerprint(row);
        let occurrence = *occurrence_by_content.get(&content).unwrap_or(&0);
        occurrence_by_content.insert(content, occurrence + 1);
        out.push(transaction_fingerprint(row, occurrence));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_types::MinorAmount;

    fn row(date: &str, text: &str, minor: i64) -> BankRow {
        BankRow {
            date: format!("{date}T00:00:00Z").parse().unwrap(),
            text: text.into(),
            amount_minor: MinorAmount::from_minor(minor),
        }
    }

    #[test]
    fn source_hash_is_stable_sha256_hex() {
        let h = source_file_hash(b"Dato;Tekst;Belob\n");
        assert_eq!(h.len(), 64);
        assert_eq!(h, source_file_hash(b"Dato;Tekst;Belob\n"));
        assert_ne!(h, source_file_hash(b"other"));
    }

    #[test]
    fn batch_id_matches_original_shape() {
        let as_of = "2026-05-20T14:30:00Z".parse::<DateTime<Utc>>().unwrap();
        let id = make_import_batch_id(as_of, "abcdef0123456789");
        assert_eq!(id, "BANK-20260520143000-abcdef01");
        assert!(id.starts_with("BANK-"));
    }

    #[test]
    fn identical_rows_get_distinct_occurrence_fingerprints() {
        let rows = [
            row("2026-05-20", "Fee", 5000),
            row("2026-05-20", "Fee", 5000),
        ];
        let fps = fingerprints_for_rows(&rows);
        assert_eq!(fps.len(), 2);
        assert_ne!(fps[0], fps[1]);
        // Re-computing yields the same pair (deterministic on re-import).
        assert_eq!(fps, fingerprints_for_rows(&rows));
    }

    #[test]
    fn text_trim_is_part_of_fingerprint() {
        let a = transaction_fingerprint(&row("2026-05-20", "Pay", 100), 0);
        let b = transaction_fingerprint(&row("2026-05-20", "  Pay  ", 100), 0);
        assert_eq!(a, b);
    }
}
