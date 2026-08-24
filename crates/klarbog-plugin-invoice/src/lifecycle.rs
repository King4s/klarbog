//! Status transitions and payment journal suggestions (no posting).
//!
//! # Payment ledger
//!
//! Payments persist on the invoice as `payments: [{unix_ms, amount_minor, currency}]`.
//! Remaining balance = `total_minor − sum(payments.amount_minor)` (i64 only).
//! - [`mark_part_paid_preview`]: records a partial (`> 0` and `< remaining`); status
//!   `part_paid`; journal suggestion for that amount. Overpay rejected.
//! - [`mark_paid_preview`]: records and suggests the **remaining** balance; rejects
//!   when remaining is 0; status `paid`.

use crate::draft::{payment_journal_suggestion_amount, InvoiceConfig};
use crate::status::InvoiceStatus;
use crate::{Invoice, InvoiceError, InvoiceId, InvoicePayment};
use chrono::Utc;
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

/// Append a credit-note ledger row after the journal is posted. Voids the
/// invoice only when cumulative credits reach original gross; otherwise
/// stays sent (partial credit). Fail-closed: sent, no payments, amount
/// within remaining.
#[allow(clippy::too_many_arguments)]
pub fn record_credit_note(
    company: &Path,
    id: &InvoiceId,
    credit_note_no: &str,
    net_minor: i64,
    vat_minor: i64,
    gross_minor: i64,
    document_id: Option<String>,
    sha256: Option<String>,
) -> Result<Invoice, InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    if !invoice.status.allows_credit() || !invoice.payments.is_empty() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: InvoiceStatus::Void,
        });
    }
    let remaining = invoice.creditable_remaining_minor()?;
    if gross_minor <= 0 || gross_minor > remaining {
        return Err(InvoiceError::CreditExceedsRemaining {
            amount_minor: gross_minor,
            remaining_minor: remaining,
        });
    }
    invoice.credits.push(crate::InvoiceCredit {
        unix_ms: Utc::now().timestamp_millis(),
        credit_note_no: credit_note_no.to_string(),
        gross_minor,
        net_minor,
        vat_minor,
        document_id,
        sha256,
    });
    invoice.credit_note_no = Some(credit_note_no.to_string());
    if invoice.creditable_remaining_minor()? == 0 {
        invoice.status = InvoiceStatus::Void;
    }
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok(updated)
}

fn push_payment(invoice: &mut Invoice, amount_minor: i64) -> Result<(), InvoiceError> {
    invoice.validate_lines()?;
    let currency = invoice.lines[0].currency.clone();
    invoice.payments.push(InvoicePayment {
        unix_ms: Utc::now().timestamp_millis(),
        amount_minor,
        currency,
    });
    Ok(())
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
    let remaining = invoice.remaining_minor()?;
    if remaining == 0 {
        return Err(InvoiceError::NothingRemaining);
    }
    if remaining < 0 {
        return Err(InvoiceError::Overpay {
            amount_minor: 0,
            remaining_minor: remaining,
        });
    }
    let entry = payment_journal_suggestion_amount(invoice, remaining, actor, cfg)?;
    push_payment(invoice, remaining)?;
    invoice.status = InvoiceStatus::Paid;
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok((updated, entry))
}

/// Record an already-booked payment on the invoice (e.g. bank reconcile
/// commit): append ledger row and derive status. No journal suggestion —
/// the entry was committed elsewhere. Fail-closed on overpay/zero.
pub fn record_payment(
    company: &Path,
    id: &InvoiceId,
    amount_minor: i64,
) -> Result<Invoice, InvoiceError> {
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
    let remaining = invoice.remaining_minor()?;
    if amount_minor <= 0 || amount_minor > remaining {
        return Err(InvoiceError::Overpay {
            amount_minor,
            remaining_minor: remaining,
        });
    }
    push_payment(invoice, amount_minor)?;
    invoice.status = if amount_minor == remaining {
        InvoiceStatus::Paid
    } else {
        InvoiceStatus::PartPaid
    };
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok(updated)
}

/// Partial payment preview: append ledger row, status → `part_paid`.
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
    let remaining = invoice.remaining_minor()?;
    if amount_minor > remaining {
        return Err(InvoiceError::Overpay {
            amount_minor,
            remaining_minor: remaining,
        });
    }
    if amount_minor <= 0 || amount_minor >= remaining {
        return Err(InvoiceError::InvalidPartialAmount {
            amount_minor,
            remaining_minor: remaining,
        });
    }
    let entry = payment_journal_suggestion_amount(invoice, amount_minor, actor, cfg)?;
    push_payment(invoice, amount_minor)?;
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
    fn part_paid_then_mark_paid_suggests_remaining() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Buyer".into(),
            klarbog_plugin_crm::PartyKind::Private,
            None,
            None,
        )
        .unwrap();
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
        assert_eq!(partial.payments.len(), 1);
        assert_eq!(partial.payments[0].amount_minor, 4_000);
        assert_eq!(partial.remaining_minor().unwrap(), 6_000);
        assert_eq!(entry.legs[0].amount.minor(), 4_000);
        assert!(entry
            .legs
            .iter()
            .all(|l| l.party_id.as_ref() == Some(&party.id)));

        let (paid, rest) = mark_paid_preview(&co, &inv.id, &actor, &cfg).unwrap();
        assert_eq!(paid.status, InvoiceStatus::Paid);
        assert_eq!(paid.payments.len(), 2);
        assert_eq!(paid.payments[1].amount_minor, 6_000);
        assert_eq!(paid.remaining_minor().unwrap(), 0);
        assert_eq!(rest.legs[0].amount.minor(), 6_000);
    }

    #[test]
    fn part_paid_rejects_non_positive_full_and_overpay() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Buyer".into(),
            klarbog_plugin_crm::PartyKind::Private,
            None,
            None,
        )
        .unwrap();
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
            Err(InvoiceError::Overpay { .. })
        ));
        mark_part_paid_preview(&co, &inv.id, 3_000, &actor, &cfg).unwrap();
        assert!(matches!(
            mark_part_paid_preview(&co, &inv.id, 2_500, &actor, &cfg),
            Err(InvoiceError::Overpay {
                amount_minor: 2_500,
                remaining_minor: 2_000,
            })
        ));
    }

    #[test]
    fn mark_paid_rejects_when_remaining_zero() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Buyer".into(),
            klarbog_plugin_crm::PartyKind::Private,
            None,
            None,
        )
        .unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Work".into(),
                amount_minor: 1_000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        patch_status(&co, &inv.id, InvoiceStatus::Sent).unwrap();
        let mut file = crate::store::load(&co).unwrap();
        let stored = file.invoices.iter_mut().find(|i| i.id == inv.id).unwrap();
        stored.payments.push(InvoicePayment {
            unix_ms: 1,
            amount_minor: 1_000,
            currency: stored.lines[0].currency.clone(),
        });
        stored.status = InvoiceStatus::PartPaid;
        crate::store::save(&co, &file).unwrap();
        let err = mark_paid_preview(&co, &inv.id, &Actor::user("t"), &InvoiceConfig::default())
            .unwrap_err();
        assert!(matches!(err, InvoiceError::NothingRemaining));
    }

    #[test]
    fn part_paid_rejects_draft() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Buyer".into(),
            klarbog_plugin_crm::PartyKind::Private,
            None,
            None,
        )
        .unwrap();
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
