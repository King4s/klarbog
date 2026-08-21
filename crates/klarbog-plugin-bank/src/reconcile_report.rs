//! Periode-afstemningsrapport (DK-BOOKKEEPING-RECONCILIATION-001, §11):
//! matchede og umatchede importerede banktransaktioner for en periode —
//! porteret fra originalens buildBankReconciliationReport. Originalen
//! matcher via FK (journal_entries.source_bank_transaction_id); portens
//! sporbare reference er memo-konventionen `bank:{text}:...` + dato.

use crate::csv::BankRow;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const RECONCILIATION_RULE_ID: &str = "DK-BOOKKEEPING-RECONCILIATION-001";

/// Posteret journalreference (kun bank-relaterede entries: memo `bank:...`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankPostedRef {
    pub as_of_date: NaiveDate,
    pub memo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconciliationRow {
    pub date: NaiveDate,
    pub text: String,
    pub amount_minor: i64,
    /// Memo på den posterede entry der matcher (tom for umatchede).
    pub matched_memo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconciliationReport {
    pub applied_rules: Vec<String>,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub matched: Vec<ReconciliationRow>,
    pub unmatched: Vec<ReconciliationRow>,
    pub matched_amount_minor: i64,
    pub unmatched_amount_minor: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReconcileReportError {
    #[error("period_start must be before or equal to period_end")]
    PeriodInverted,
}

/// Byg rapporten for `rows` inden for perioden. En række er matched når en
/// posteret entry har samme dato og memo der starter med `bank:{text}:`;
/// hver posteret reference forbruges højst én gang (spejler originalens
/// 1:1 FK-match).
pub fn reconciliation_report(
    rows: &[BankRow],
    posted: &[BankPostedRef],
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> Result<ReconciliationReport, ReconcileReportError> {
    if period_start > period_end {
        return Err(ReconcileReportError::PeriodInverted);
    }
    let mut consumed = vec![false; posted.len()];
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    let mut matched_amount_minor = 0i64;
    let mut unmatched_amount_minor = 0i64;

    let mut in_period: Vec<&BankRow> = rows
        .iter()
        .filter(|r| {
            let d = r.date.date_naive();
            d >= period_start && d <= period_end
        })
        .collect();
    in_period.sort_by_key(|r| r.date);

    for row in in_period {
        let date = row.date.date_naive();
        let prefix = format!("bank:{}:", row.text);
        let hit = posted
            .iter()
            .enumerate()
            .position(|(i, p)| !consumed[i] && p.as_of_date == date && p.memo.starts_with(&prefix));
        let amount = row.amount_minor.minor();
        match hit {
            Some(i) => {
                consumed[i] = true;
                matched_amount_minor += amount;
                matched.push(ReconciliationRow {
                    date,
                    text: row.text.clone(),
                    amount_minor: amount,
                    matched_memo: posted[i].memo.clone(),
                });
            }
            None => {
                unmatched_amount_minor += amount;
                unmatched.push(ReconciliationRow {
                    date,
                    text: row.text.clone(),
                    amount_minor: amount,
                    matched_memo: String::new(),
                });
            }
        }
    }

    Ok(ReconciliationReport {
        applied_rules: vec![RECONCILIATION_RULE_ID.to_string()],
        period_start,
        period_end,
        matched,
        unmatched,
        matched_amount_minor,
        unmatched_amount_minor,
    })
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

    fn posted(date: &str, memo: &str) -> BankPostedRef {
        BankPostedRef {
            as_of_date: date.parse().unwrap(),
            memo: memo.into(),
        }
    }

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn splits_matched_and_unmatched_with_totals() {
        let rows = [
            row("2026-05-20", "Customer payment", 50_000),
            row("2026-05-21", "Unknown transfer", -2_500),
        ];
        let posted = [posted("2026-05-20", "bank:Customer payment:invoice:INV1")];
        let report =
            reconciliation_report(&rows, &posted, d("2026-05-01"), d("2026-05-31")).unwrap();
        assert_eq!(report.applied_rules, vec![RECONCILIATION_RULE_ID]);
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.unmatched.len(), 1);
        assert_eq!(report.matched_amount_minor, 50_000);
        assert_eq!(report.unmatched_amount_minor, -2_500);
        assert_eq!(
            report.matched[0].matched_memo,
            "bank:Customer payment:invoice:INV1"
        );
    }

    #[test]
    fn rows_outside_period_are_excluded() {
        let rows = [row("2026-06-01", "Late", 100)];
        let report = reconciliation_report(&rows, &[], d("2026-05-01"), d("2026-05-31")).unwrap();
        assert!(report.matched.is_empty());
        assert!(report.unmatched.is_empty());
    }

    #[test]
    fn posted_ref_is_consumed_once() {
        // To ens rækker, én postering: kun én må matche (originalens 1:1 FK).
        let rows = [row("2026-05-20", "Dup", 100), row("2026-05-20", "Dup", 100)];
        let posted = [posted("2026-05-20", "bank:Dup:2026-05-20")];
        let report =
            reconciliation_report(&rows, &posted, d("2026-05-01"), d("2026-05-31")).unwrap();
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.unmatched.len(), 1);
    }

    #[test]
    fn date_mismatch_does_not_match() {
        let rows = [row("2026-05-20", "Pay", 100)];
        let posted = [posted("2026-05-21", "bank:Pay:invoice:X")];
        let report =
            reconciliation_report(&rows, &posted, d("2026-05-01"), d("2026-05-31")).unwrap();
        assert!(report.matched.is_empty());
        assert_eq!(report.unmatched.len(), 1);
    }

    #[test]
    fn inverted_period_is_rejected() {
        let err = reconciliation_report(&[], &[], d("2026-06-01"), d("2026-05-01")).unwrap_err();
        assert_eq!(err, ReconcileReportError::PeriodInverted);
    }
}
