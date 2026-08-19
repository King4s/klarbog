//! Bank reconciliation — suggest matches between bank rows and open invoice drafts.
//! Read-only: never posts journal entries (ADR-006).

use crate::csv::BankRow;
use klarbog_plugin_crm::{get_party, CrmError};
use klarbog_plugin_documents::{
    list_exceptions, raise_exception, DocumentError, Exception, ExceptionSeverity,
};
use klarbog_plugin_invoice::{
    journal_suggestion, list_invoices, Invoice, InvoiceConfig, InvoiceError, InvoiceKind,
    InvoiceStatus,
};
use klarbog_types::Actor;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

/// Reused documents exception code (snake_case mirror of upstream UNMATCHED_BANK_TRANSACTION).
pub const EXCEPTION_UNMATCHED_BANK: &str = "unmatched_bank_transaction";

/// Safe auto-apply threshold: 50.00% expressed as basis points (no floats).
pub const SAFE_THRESHOLD_BPS: u32 = 5000;
const AMOUNT_MATCH_SALE_BPS: u32 = 6000;
const AMOUNT_MATCH_PURCHASE_BPS: u32 = 5500;
const INVOICE_ID_IN_TEXT_BPS: u32 = 2500;
const MEMO_TOKEN_BPS: u32 = 1000;
const NAME_TOKEN_EACH_BPS: u32 = 500;
const NAME_TOKEN_CAP_BPS: u32 = 1500;
const LOW_CONFIDENCE_CAP_BPS: u32 = 4500;

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
}

struct OpenTarget {
    invoice: Invoice,
    party_name: String,
    journal_memo: String,
    total_minor: i64,
}

fn stop_tokens() -> HashSet<&'static str> {
    [
        "APS", "A/S", "AS", "IVS", "P/S", "PS", "K/S", "KS", "I/S", "IS", "AMBA", "FMBA", "SMBA",
        "GMBH", "LTD", "INC", "PLC", "LLC", "AB", "OY", "DKK", "EUR", "USD", "SEK", "NOK", "GBP",
        "FOR", "OG", "THE", "AND", "VED", "MED", "TIL", "FRA", "DEN", "DET",
    ]
    .into_iter()
    .collect()
}

fn tokenize(value: &str) -> Vec<String> {
    let stops = stop_tokens();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let upper = value.to_uppercase();
    for token in upper.split(|c: char| !c.is_alphanumeric() && c != '-') {
        let t = token.trim();
        if t.len() < 3 || stops.contains(t) {
            continue;
        }
        if seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    }
    out
}

fn overlap_tokens(left: &str, right: &str) -> Vec<String> {
    let left_set: HashSet<_> = tokenize(left).into_iter().collect();
    tokenize(right)
        .into_iter()
        .filter(|t| left_set.contains(t))
        .collect()
}

fn bank_text(row: &BankRow) -> String {
    row.text.to_uppercase()
}

fn load_open_targets(company: &Path) -> Result<Vec<OpenTarget>, ReconcileError> {
    let actor = Actor::agent("bank-reconcile");
    let cfg = InvoiceConfig::default();
    let mut out = Vec::new();
    for invoice in list_invoices(company)? {
        if matches!(invoice.status, InvoiceStatus::Paid | InvoiceStatus::Void) {
            continue;
        }
        let party_name = get_party(company, &invoice.party_id)?
            .map(|p| p.display_name)
            .unwrap_or_else(|| invoice.party_id.to_string());
        let journal_memo = journal_suggestion(&invoice, &actor, &cfg)
            .map(|e| e.memo)
            .unwrap_or_default();
        let total_minor = invoice.total_minor()?;
        out.push(OpenTarget {
            invoice,
            party_name,
            journal_memo,
            total_minor,
        });
    }
    Ok(out)
}

fn score_match(row: &BankRow, target: &OpenTarget) -> Option<MatchSuggestion> {
    let signed = row.amount_minor.minor();
    let payment_minor = signed.unsigned_abs() as i64;
    if payment_minor == 0 {
        return None;
    }

    let kind_ok = match target.invoice.kind {
        InvoiceKind::Sale if signed > 0 => true,
        InvoiceKind::Purchase if signed < 0 => true,
        _ => false,
    };
    if !kind_ok {
        return None;
    }

    let text = bank_text(row);
    let mut confidence_bps = 0u32;
    let mut reasons = Vec::new();
    let mut corroborated = false;
    let mut memo_match = false;

    if payment_minor == target.total_minor {
        confidence_bps += match target.invoice.kind {
            InvoiceKind::Sale => AMOUNT_MATCH_SALE_BPS,
            InvoiceKind::Purchase => AMOUNT_MATCH_PURCHASE_BPS,
        };
        reasons.push(format!(
            "amount match: {} vs invoice total {}",
            payment_minor, target.total_minor
        ));
    } else {
        return None;
    }

    let inv_id = target.invoice.id.as_str();
    if text.contains(&inv_id.to_uppercase()) {
        confidence_bps += INVOICE_ID_IN_TEXT_BPS;
        corroborated = true;
        reasons.push(format!("invoice id '{inv_id}' found in bank text"));
    }

    let name_tokens = overlap_tokens(&text, &target.party_name);
    if !name_tokens.is_empty() {
        let add = (name_tokens.len() as u32 * NAME_TOKEN_EACH_BPS).min(NAME_TOKEN_CAP_BPS);
        confidence_bps += add;
        if name_tokens.len() >= 2 {
            corroborated = true;
        }
        reasons.push(format!("party token match: {}", name_tokens.join(", ")));
    }

    let memo_tokens = overlap_tokens(&text, &target.journal_memo);
    if memo_tokens.len() >= 2 {
        confidence_bps += MEMO_TOKEN_BPS;
        corroborated = true;
        memo_match = true;
        reasons.push(format!(
            "journal memo token match: {}",
            memo_tokens.join(", ")
        ));
    }

    if !corroborated && confidence_bps >= SAFE_THRESHOLD_BPS {
        confidence_bps = LOW_CONFIDENCE_CAP_BPS;
        reasons
            .push("low confidence: amount-only match, no invoice id or name corroboration".into());
    }

    if confidence_bps < SAFE_THRESHOLD_BPS {
        return None;
    }

    let kind = if memo_match && name_tokens.is_empty() && !text.contains(&inv_id.to_uppercase()) {
        MatchKind::JournalMemo
    } else {
        match target.invoice.kind {
            InvoiceKind::Sale => MatchKind::SaleInvoice,
            InvoiceKind::Purchase => MatchKind::PurchaseInvoice,
        }
    };

    Some(MatchSuggestion {
        kind,
        invoice_id: inv_id.to_string(),
        party_name: Some(target.party_name.clone()),
        memo: Some(target.journal_memo.clone()),
        confidence_bps,
        reasons,
    })
}

fn exact_amount_unsafe_reason(row: &BankRow, targets: &[OpenTarget]) -> Option<String> {
    let signed = row.amount_minor.minor();
    let payment_minor = signed.unsigned_abs() as i64;
    for target in targets {
        let kind_ok = match target.invoice.kind {
            InvoiceKind::Sale if signed > 0 => true,
            InvoiceKind::Purchase if signed < 0 => true,
            _ => false,
        };
        if kind_ok && payment_minor == target.total_minor {
            return Some(format!(
                "exact amount matches invoice {} but lacks corroborating text",
                target.invoice.id
            ));
        }
    }
    None
}

/// Score bank rows against open invoice drafts and their journal suggestion memos.
pub fn suggest_matches(
    company: &Path,
    bank_rows: &[BankRow],
) -> Result<Vec<BankRowMatchResult>, ReconcileError> {
    let targets = load_open_targets(company)?;
    let mut results = Vec::with_capacity(bank_rows.len());
    for (row_index, row) in bank_rows.iter().enumerate() {
        let mut suggestions: Vec<MatchSuggestion> =
            targets.iter().filter_map(|t| score_match(row, t)).collect();
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

fn bank_row_related_id(row_index: usize, row: &BankRow) -> String {
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
