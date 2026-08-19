//! Bank reconciliation — suggest matches between bank rows and open invoice drafts.
//! Read-only: never posts journal entries (ADR-006).

use crate::csv::BankRow;
use klarbog_plugin_crm::CrmError;
use klarbog_plugin_documents::{
    list_exceptions, raise_exception, DocumentError, Exception, ExceptionSeverity,
};
use klarbog_plugin_invoice::InvoiceError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[path = "reconcile_score.rs"]
mod score;

use score::{exact_amount_unsafe_reason, load_open_targets};
pub(crate) use score::{open_target_for_invoice, score_match};

/// Reused documents exception code (snake_case mirror of upstream UNMATCHED_BANK_TRANSACTION).
pub const EXCEPTION_UNMATCHED_BANK: &str = "unmatched_bank_transaction";

/// Safe auto-apply threshold: 50.00% expressed as basis points (no floats).
pub const SAFE_THRESHOLD_BPS: u32 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    SaleInvoice,
    PurchaseInvoice,
    JournalMemo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSuggestion {
    pub kind: MatchKind,
    pub invoice_id: String,
    pub party_name: Option<String>,
    pub memo: Option<String>,
    /// Confidence 0–10_000 basis points (5000 = 50%).
    pub confidence_bps: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankRowMatchResult {
    pub row_index: usize,
    pub date: String,
    pub text: String,
    pub amount_minor: i64,
    pub suggestions: Vec<MatchSuggestion>,
    /// Set when an exact-amount candidate exists but confidence stays below the safe threshold.
    pub unsafe_match_reason: Option<String>,
}

#[derive(Debug, Error)]
pub enum ReconcileError {
    #[error(transparent)]
    Invoice(#[from] InvoiceError),
    #[error(transparent)]
    Crm(#[from] CrmError),
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error("invoice not found: {0}")]
    InvoiceNotFound(String),
    #[error("invoice status {0} cannot be reconciled")]
    InvoiceNotOpen(String),
    #[error("bank row amount/kind does not match invoice")]
    AmountMismatch,
    #[error(
        "match confidence {confidence_bps} below safe threshold {threshold_bps}; set force=true as user to override"
    )]
    UnsafeMatch {
        confidence_bps: u32,
        threshold_bps: u32,
    },
    #[error("force apply requires user actor")]
    ForceRequiresUser,
}

/// Score bank rows against open invoice drafts and their journal suggestion memos.
pub fn suggest_matches(
    company: &Path,
    bank_rows: &[BankRow],
) -> Result<Vec<BankRowMatchResult>, ReconcileError> {
    let targets = load_open_targets(company)?;
    let mut results = Vec::with_capacity(bank_rows.len());
    for (row_index, row) in bank_rows.iter().enumerate() {
        let mut suggestions: Vec<MatchSuggestion> = targets
            .iter()
            .filter_map(|t| score_match(row, t, false))
            .collect();
        suggestions.sort_by_key(|b| std::cmp::Reverse(b.confidence_bps));
        let unsafe_match_reason = if suggestions.is_empty() {
            exact_amount_unsafe_reason(row, &targets)
        } else {
            None
        };
        results.push(BankRowMatchResult {
            row_index,
            date: row.date.format("%Y-%m-%d").to_string(),
            text: row.text.clone(),
            amount_minor: row.amount_minor.minor(),
            suggestions,
            unsafe_match_reason,
        });
    }
    Ok(results)
}

pub(crate) fn bank_row_related_id(row_index: usize, row: &BankRow) -> String {
    format!(
        "bank_row:{row_index}:{}:{}:{}",
        row.date.format("%Y-%m-%d"),
        row.amount_minor.minor(),
        row.text
    )
}

fn unmatched_message(row: &BankRow, unsafe_reason: Option<&str>) -> String {
    let signed = row.amount_minor.minor();
    let direction = if signed > 0 {
        "incoming payment"
    } else {
        "outgoing payment"
    };
    let base = format!(
        "Bank line \"{}\" on {} ({}) is unmatched",
        row.text,
        row.date.format("%Y-%m-%d"),
        direction
    );
    match unsafe_reason {
        Some(r) => format!("{base}: {r}"),
        None => format!("{base}: no matching open invoice draft found"),
    }
}

/// Raise open exceptions for bank rows without a safe suggestion; idempotent per related id.
pub fn sync_unmatched_exceptions(
    company: &Path,
    bank_rows: &[BankRow],
    results: &[BankRowMatchResult],
) -> Result<Vec<Exception>, ReconcileError> {
    let open = list_exceptions(company, true)?;
    let mut raised = Vec::new();
    for (row_index, row) in bank_rows.iter().enumerate() {
        let Some(result) = results.get(row_index) else {
            continue;
        };
        if !result.suggestions.is_empty() {
            continue;
        }
        let related = bank_row_related_id(row_index, row);
        let already = open.iter().any(|e| {
            e.code == EXCEPTION_UNMATCHED_BANK
                && e.open
                && e.related_ids.iter().any(|r| r == &related)
        });
        if already {
            continue;
        }
        let message = unmatched_message(row, result.unsafe_match_reason.as_deref());
        let exc = raise_exception(
            company,
            EXCEPTION_UNMATCHED_BANK.into(),
            ExceptionSeverity::Warn,
            message,
            vec![related],
        )?;
        raised.push(exc);
    }
    Ok(raised)
}

/// List open unmatched-bank exceptions for a company.
pub fn list_unmatched_bank_exceptions(company: &Path) -> Result<Vec<Exception>, ReconcileError> {
    Ok(list_exceptions(company, true)?
        .into_iter()
        .filter(|e| e.code == EXCEPTION_UNMATCHED_BANK)
        .collect())
}
