//! Scoring helpers for bank↔invoice reconciliation (private to `reconcile`).

use super::{MatchKind, MatchSuggestion, ReconcileError, SAFE_THRESHOLD_BPS};
use crate::csv::BankRow;
use klarbog_plugin_crm::get_party;
use klarbog_plugin_invoice::{
    journal_suggestion, list_invoices, Invoice, InvoiceConfig, InvoiceKind, InvoiceStatus,
};
use klarbog_types::Actor;
use std::collections::HashSet;
use std::path::Path;

const AMOUNT_MATCH_SALE_BPS: u32 = 6000;
const AMOUNT_MATCH_PURCHASE_BPS: u32 = 5500;
const INVOICE_ID_IN_TEXT_BPS: u32 = 2500;
const MEMO_TOKEN_BPS: u32 = 1000;
const NAME_TOKEN_EACH_BPS: u32 = 500;
const NAME_TOKEN_CAP_BPS: u32 = 1500;
const LOW_CONFIDENCE_CAP_BPS: u32 = 4500;

pub(crate) struct OpenTarget {
    pub(crate) invoice: Invoice,
    pub(crate) party_name: String,
    pub(crate) journal_memo: String,
    pub(crate) total_minor: i64,
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

pub(super) fn load_open_targets(company: &Path) -> Result<Vec<OpenTarget>, ReconcileError> {
    let mut out = Vec::new();
    for invoice in list_invoices(company)? {
        if matches!(invoice.status, InvoiceStatus::Paid | InvoiceStatus::Void) {
            continue;
        }
        out.push(open_target_for_invoice(company, invoice)?);
    }
    Ok(out)
}

/// Score a row/target pair. When `allow_below_safe` is false, drops suggestions under
/// [`SAFE_THRESHOLD_BPS`] (suggest path). Apply uses `true` so unsafe matches can be forced.
pub(crate) fn score_match(
    row: &BankRow,
    target: &OpenTarget,
    allow_below_safe: bool,
) -> Option<MatchSuggestion> {
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

    if !allow_below_safe && confidence_bps < SAFE_THRESHOLD_BPS {
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

pub(super) fn exact_amount_unsafe_reason(row: &BankRow, targets: &[OpenTarget]) -> Option<String> {
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

pub(crate) fn open_target_for_invoice(
    company: &Path,
    invoice: Invoice,
) -> Result<OpenTarget, ReconcileError> {
    let actor = Actor::agent("bank-reconcile");
    let cfg = InvoiceConfig::default();
    let party_name = get_party(company, &invoice.party_id)?
        .map(|p| p.display_name)
        .unwrap_or_else(|| invoice.party_id.to_string());
    let journal_memo = journal_suggestion(&invoice, &actor, &cfg)
        .map(|e| e.memo)
        .unwrap_or_default();
    let total_minor = invoice.total_minor()?;
    Ok(OpenTarget {
        invoice,
        party_name,
        journal_memo,
        total_minor,
    })
}
