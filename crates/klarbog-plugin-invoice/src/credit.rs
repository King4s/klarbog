//! Credit note journal suggestion (ADR-020 / DK-CREDIT-NOTE-001).

use crate::draft::{journal_suggestion, InvoiceConfig};
use crate::{Invoice, InvoiceError, InvoiceStatus};
use chrono::Utc;
use klarbog_journal::JournalEntry;
use klarbog_types::Actor;

/// Full credit note: exact negation of the send booking — same accounts
/// (incl. VAT leg), flipped directions, memo
/// `invoice:{id}:credit:{CN-nr} · {reason}`. As in the original project a
/// reason is required and the CN number is sequential per fiscal year
/// (DK-CREDIT-NOTE-001). Fail-closed: only a sent invoice with no recorded
/// payments; partial credit notes are not yet ported (see ADR-020).
pub fn credit_journal_suggestion(
    invoice: &Invoice,
    credit_note_no: &str,
    reason: &str,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<JournalEntry, InvoiceError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(InvoiceError::MissingCreditReason);
    }
    if invoice.status != InvoiceStatus::Sent {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: InvoiceStatus::Void,
        });
    }
    if !invoice.payments.is_empty() {
        return Err(InvoiceError::CreditWithPayments);
    }
    let entry = journal_suggestion(invoice, actor, cfg)?;
    // CN-nummeret bages ind i det digest-bundne memo; commit reserverer
    // præcis dette nummer (fail-closed ved kapløb, som originalens
    // reserveSequenceValue).
    let credit = entry.reversal(
        Utc::now(),
        actor.clone(),
        format!("invoice:{}:credit:{credit_note_no} · {reason}", invoice.id),
    );
    credit.validate().map_err(InvoiceError::Journal)?;
    Ok(credit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::vat_invoice;
    use crate::InvoiceKind;
    use klarbog_journal::Direction;
    use klarbog_types::{Actor, Currency};

    #[test]
    fn credit_note_negates_send_booking_incl_vat() {
        let mut inv = vat_invoice(InvoiceKind::Sale);
        inv.status = InvoiceStatus::Sent;
        let entry = credit_journal_suggestion(
            &inv,
            "CN-2026-0001",
            "forkert beløb",
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap();
        assert_eq!(
            entry.memo,
            format!("invoice:{}:credit:CN-2026-0001 · forkert beløb", inv.id)
        );
        assert_eq!(entry.legs.len(), 3);
        // AR credited gross, revenue debited net, salgsmoms debited vat.
        assert_eq!(entry.legs[0].account, "1100");
        assert_eq!(entry.legs[0].direction, Direction::Credit);
        assert_eq!(entry.legs[0].amount.minor(), 31_250);
        assert_eq!(entry.legs[1].account, "1000");
        assert_eq!(entry.legs[1].direction, Direction::Debit);
        assert_eq!(entry.legs[1].amount.minor(), 25_000);
        assert_eq!(entry.legs[2].account, "1200");
        assert_eq!(entry.legs[2].direction, Direction::Debit);
        assert_eq!(entry.legs[2].amount.minor(), 6_250);
    }

    #[test]
    fn credit_note_refuses_draft_and_paid_invoices() {
        let draft = vat_invoice(InvoiceKind::Sale);
        let err = credit_journal_suggestion(
            &draft,
            "CN-2026-0001",
            "fejl",
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(err, InvoiceError::InvalidTransition { .. }));

        let mut with_payment = vat_invoice(InvoiceKind::Sale);
        with_payment.status = InvoiceStatus::Sent;
        with_payment.payments.push(crate::InvoicePayment {
            unix_ms: 0,
            amount_minor: 1_000,
            currency: Currency::new("DKK").unwrap(),
        });
        let err = credit_journal_suggestion(
            &with_payment,
            "CN-2026-0001",
            "fejl",
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(err, InvoiceError::CreditWithPayments));
    }

    #[test]
    fn credit_note_requires_reason_as_in_original() {
        let mut inv = vat_invoice(InvoiceKind::Sale);
        inv.status = InvoiceStatus::Sent;
        let err = credit_journal_suggestion(
            &inv,
            "CN-2026-0001",
            "  ",
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(err, InvoiceError::MissingCreditReason));
    }
}
