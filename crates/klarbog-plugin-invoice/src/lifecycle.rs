//! Status transitions and payment journal suggestions (no posting).
//!
//! # DEV model (no payment ledger)
//!
//! Cumulative paid amounts are not tracked. Simplification:
//! - [`mark_part_paid_preview`]: requires `sent|part_paid`; `amount_minor` must be
//!   `> 0` and `< invoice.total_minor()`; sets status `part_paid`; journal suggestion
//!   for that partial amount only.
//! - [`mark_paid_preview`]: requires `sent|part_paid`; journal suggestion always for
//!   the **full invoice total** (not a computed remaining); sets status `paid`.
//!
//! A later payment ledger can introduce true remaining / multi-partial accounting.

use crate::draft::{payment_journal_suggestion, payment_journal_suggestion_amount, InvoiceConfig};
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
    // DEV: always full total — no remaining without a payment ledger.
    let entry = payment_journal_suggestion(invoice, actor, cfg)?;
    invoice.status = InvoiceStatus::Paid;
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok((updated, entry))
}

/// Partial payment preview: status → `part_paid`, journal suggestion for `amount_minor`.
pub fn mark_part_paid_preview(
    company: &Path,
    id: &InvoiceId,
    amount_minor: i64,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<(Invoice, JournalEntry), InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    if !invoice.status.allows_mark_part_paid() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: InvoiceStatus::PartPaid,
        });
    }
    let total = invoice.total_minor()?;
    // DEV: no remaining ledger — validate against invoice total only.
    if amount_minor <= 0 || amount_minor >= total {
        return Err(InvoiceError::InvalidPartialAmount {
            amount_minor,
            total_minor: total,
        });
    }
    let entry = payment_journal_suggestion_amount(invoice, amount_minor, actor, cfg)?;
    invoice.status = InvoiceStatus::PartPaid;
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok((updated, entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{create_draft_from_new, NewLine};
    use crate::InvoiceKind;
    use klarbog_plugin_crm::upsert_party;
    use tempfile::tempdir;

    #[test]
    fn part_paid_then_mark_paid_full_total() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Buyer".into()).unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id.clone(),
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Work".into(),
                amount_minor: 10_000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        patch_status(&co, &inv.id, InvoiceStatus::Sent).unwrap();
        let actor = Actor::user("t");
        let cfg = InvoiceConfig::default();
        let (partial, entry) = mark_part_paid_preview(&co, &inv.id, 4_000, &actor, &cfg).unwrap();
        assert_eq!(partial.status, InvoiceStatus::PartPaid);
        assert_eq!(entry.legs[0].amount.minor(), 4_000);
        assert!(entry
            .legs
            .iter()
            .all(|l| l.party_id.as_ref() == Some(&party.id)));

        let (paid, full) = mark_paid_preview(&co, &inv.id, &actor, &cfg).unwrap();
        assert_eq!(paid.status, InvoiceStatus::Paid);
        assert_eq!(full.legs[0].amount.minor(), 10_000);
    }

    #[test]
    fn part_paid_rejects_non_positive_and_full_or_over() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Buyer".into()).unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Work".into(),
                amount_minor: 5_000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        patch_status(&co, &inv.id, InvoiceStatus::Sent).unwrap();
        let actor = Actor::user("t");
        let cfg = InvoiceConfig::default();
        assert!(matches!(
            mark_part_paid_preview(&co, &inv.id, 0, &actor, &cfg),
            Err(InvoiceError::InvalidPartialAmount { .. })
        ));
        assert!(matches!(
            mark_part_paid_preview(&co, &inv.id, 5_000, &actor, &cfg),
            Err(InvoiceError::InvalidPartialAmount { .. })
        ));
        assert!(matches!(
            mark_part_paid_preview(&co, &inv.id, 5_001, &actor, &cfg),
            Err(InvoiceError::InvalidPartialAmount { .. })
        ));
    }

    #[test]
    fn part_paid_rejects_draft() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Buyer".into()).unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Work".into(),
                amount_minor: 5_000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        let err = mark_part_paid_preview(
            &co,
            &inv.id,
            1_000,
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            InvoiceError::InvalidTransition {
                from: InvoiceStatus::Draft,
                to: InvoiceStatus::PartPaid,
            }
        ));
    }
}
