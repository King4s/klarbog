//! Apply a bank↔invoice match as a journal **preview** suggestion only (no post).

use crate::csv::BankRow;
use crate::reconcile::{
    bank_row_related_id, open_target_for_invoice, score_match, ReconcileError,
    EXCEPTION_UNMATCHED_BANK, SAFE_THRESHOLD_BPS,
};
use klarbog_journal::JournalEntry;
use klarbog_plugin_documents::{list_exceptions, set_exception_open, Exception};
use klarbog_plugin_invoice::{
    get_invoice, payment_journal_suggestion, InvoiceConfig, InvoiceId, InvoiceStatus,
};
use klarbog_types::{Actor, ActorKind};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyMatchResult {
    pub entry: JournalEntry,
    pub confidence_bps: u32,
    pub forced: bool,
    pub exception_closed: Option<Exception>,
}

/// Build a payment journal suggestion for a bank row ↔ invoice match.
/// Does **not** post and does **not** mark the invoice paid (host two-phase commit).
pub fn apply_match(
    company: &Path,
    bank_row: &BankRow,
    invoice_id: &str,
    actor: &Actor,
    force: bool,
    row_index: usize,
) -> Result<ApplyMatchResult, ReconcileError> {
    let id = InvoiceId::new(invoice_id);
    let invoice = get_invoice(company, &id)?
        .ok_or_else(|| ReconcileError::InvoiceNotFound(invoice_id.to_string()))?;
    if matches!(invoice.status, InvoiceStatus::Paid | InvoiceStatus::Void) {
        return Err(ReconcileError::InvoiceNotOpen(format!(
            "{:?}",
            invoice.status
        )));
    }

    let target = open_target_for_invoice(company, invoice)?;
    let suggestion = score_match(bank_row, &target, true).ok_or(ReconcileError::AmountMismatch)?;
    let confidence_bps = suggestion.confidence_bps;

    let mut forced = false;
    if confidence_bps < SAFE_THRESHOLD_BPS {
        if !force {
            return Err(ReconcileError::UnsafeMatch {
                confidence_bps,
                threshold_bps: SAFE_THRESHOLD_BPS,
            });
        }
        if actor.kind != ActorKind::User {
            return Err(ReconcileError::ForceRequiresUser);
        }
        forced = true;
    }

    let cfg = InvoiceConfig::default();
    let mut entry = payment_journal_suggestion(&target.invoice, actor, &cfg)?;
    entry.as_of = bank_row.date;
    entry.memo = format!(
        "bank:{}:invoice:{}",
        bank_row.text,
        target.invoice.id.as_str()
    );
    entry
        .validate()
        .map_err(|e| ReconcileError::Invoice(klarbog_plugin_invoice::InvoiceError::from(e)))?;

    let exception_closed = close_related_unmatched(company, row_index, bank_row)?;
    Ok(ApplyMatchResult {
        entry,
        confidence_bps,
        forced,
        exception_closed,
    })
}

fn close_related_unmatched(
    company: &Path,
    row_index: usize,
    row: &BankRow,
) -> Result<Option<Exception>, ReconcileError> {
    let related = bank_row_related_id(row_index, row);
    let open = list_exceptions(company, true)?;
    let Some(exc) = open.into_iter().find(|e| {
        e.code == EXCEPTION_UNMATCHED_BANK && e.related_ids.iter().any(|r| r == &related)
    }) else {
        return Ok(None);
    };
    Ok(Some(set_exception_open(company, &exc.id, false)?))
}
