//! Status transitions and mark-paid journal suggestions (no posting).

use crate::draft::{payment_journal_suggestion, InvoiceConfig};
use crate::status::InvoiceStatus;
use crate::{Invoice, InvoiceError, InvoiceId};
use klarbog_journal::JournalEntry;
use klarbog_types::Actor;
use std::path::Path;

pub fn patch_status(
    company: &Path,
    id: &InvoiceId,
    status: InvoiceStatus,
) -> Result<Invoice, InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    if !invoice.status.allows_patch_from() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: status,
        });
    }
    invoice.status = status;
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok(updated)
}

pub fn mark_paid_preview(
    company: &Path,
    id: &InvoiceId,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<(Invoice, JournalEntry), InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    if !invoice.status.allows_mark_paid() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: InvoiceStatus::Paid,
        });
    }
    let entry = payment_journal_suggestion(invoice, actor, cfg)?;
    invoice.status = InvoiceStatus::Paid;
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok((updated, entry))
}
