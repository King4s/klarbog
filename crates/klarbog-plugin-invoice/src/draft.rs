//! Balanced journal entry suggestions for invoice drafts (no posting).

use crate::{Invoice, InvoiceError, InvoiceKind, InvoiceStatus};
use chrono::Utc;
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_types::{Actor, MinorAmount};

#[derive(Debug, Clone)]
pub struct InvoiceConfig {
    pub ar_account: String,
    pub revenue_account: String,
    pub ap_account: String,
    pub expense_account: String,
    pub bank_account: String,
    /// Salgsmoms (udgående moms) — credited on sale invoices (ADR-020).
    pub salgsmoms_account: String,
    /// Købsmoms (indgående moms) — debited on purchase invoices (ADR-020).
    pub koebsmoms_account: String,
}

impl Default for InvoiceConfig {
    fn default() -> Self {
        Self {
            ar_account: "1100".into(),
            revenue_account: "1000".into(),
            ap_account: "7000".into(),
            expense_account: "3000".into(),
            bank_account: "2000".into(),
            salgsmoms_account: "1200".into(),
            koebsmoms_account: "4000".into(),
        }
    }
}

/// Booking amounts: (net, vat, gross). Legacy invoices without `vat` book
/// the full line total with no VAT leg (vat = 0, net = gross = total).
fn booking_amounts(invoice: &Invoice) -> Result<(i64, i64, i64), InvoiceError> {
    match &invoice.vat {
        Some(v) => Ok((v.net_minor, v.vat_minor, v.gross_minor)),
        None => {
            let total = invoice.total_minor()?;
            Ok((total, 0, total))
        }
    }
}

pub fn journal_suggestion(
    invoice: &Invoice,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<JournalEntry, InvoiceError> {
    invoice.validate_lines()?;
    let (net, vat, gross) = booking_amounts(invoice)?;
    let currency = invoice.lines[0].currency.clone();
    let party = Some(invoice.party_id.clone());
    let memo = format!("invoice:{}:{}", invoice.id, invoice.lines[0].description);
    let mut legs = match invoice.kind {
        // AR carries gross (what the customer owes); revenue is net.
        InvoiceKind::Sale => vec![
            leg(
                &cfg.ar_account,
                Direction::Debit,
                MinorAmount::from_minor(gross),
                &currency,
                party.clone(),
            ),
            leg(
                &cfg.revenue_account,
                Direction::Credit,
                MinorAmount::from_minor(net),
                &currency,
                party.clone(),
            ),
        ],
        // Expense is net; AP carries gross (what we owe the supplier).
        InvoiceKind::Purchase => vec![
            leg(
                &cfg.expense_account,
                Direction::Debit,
                MinorAmount::from_minor(net),
                &currency,
                party.clone(),
            ),
            leg(
                &cfg.ap_account,
                Direction::Credit,
                MinorAmount::from_minor(gross),
                &currency,
                party.clone(),
            ),
        ],
    };
    if vat > 0 {
        let (account, direction) = match invoice.kind {
            InvoiceKind::Sale => (&cfg.salgsmoms_account, Direction::Credit),
            InvoiceKind::Purchase => (&cfg.koebsmoms_account, Direction::Debit),
        };
        legs.push(leg(
            account,
            direction,
            MinorAmount::from_minor(vat),
            &currency,
            party,
        ));
    }
    let entry = JournalEntry {
        as_of: Utc::now(),
        memo,
        legs,
        actor: actor.clone(),
    };
    entry.validate().map_err(InvoiceError::Journal)?;
    Ok(entry)
}

/// Full credit note: exact negation of the send booking — same accounts
/// (incl. VAT leg), flipped directions, memo `invoice:{id}:credit · {reason}`.
/// As in the original project a reason is required (DK-CREDIT-NOTE-001).
/// Fail-closed: only a sent invoice with no recorded payments; partial
/// credit notes and the CN number sequence are original features not yet
/// ported (see ADR-020).
pub fn credit_journal_suggestion(
    invoice: &Invoice,
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
    let credit = entry.reversal(
        Utc::now(),
        actor.clone(),
        format!("invoice:{}:credit · {reason}", invoice.id),
    );
    credit.validate().map_err(InvoiceError::Journal)?;
    Ok(credit)
}

pub fn payment_journal_suggestion(
    invoice: &Invoice,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<JournalEntry, InvoiceError> {
    // Payments settle the gross (bank amounts are always VAT-inclusive).
    let gross = invoice.gross_minor()?;
    payment_journal_suggestion_amount(invoice, gross, actor, cfg)
}

/// Payment legs for an explicit `amount_minor` (partial or full). Same account mapping as full payment.
pub fn payment_journal_suggestion_amount(
    invoice: &Invoice,
    amount_minor: i64,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<JournalEntry, InvoiceError> {
    invoice.validate_lines()?;
    if amount_minor <= 0 {
        return Err(InvoiceError::NonPositiveAmount);
    }
    let currency = invoice.lines[0].currency.clone();
    let amount = MinorAmount::from_minor(amount_minor);
    let memo = format!("invoice:{}:payment", invoice.id);
    let legs = match invoice.kind {
        InvoiceKind::Sale => vec![
            leg(
                &cfg.bank_account,
                Direction::Debit,
                amount,
                &currency,
                Some(invoice.party_id.clone()),
            ),
            leg(
                &cfg.ar_account,
                Direction::Credit,
                amount,
                &currency,
                Some(invoice.party_id.clone()),
            ),
        ],
        InvoiceKind::Purchase => vec![
            leg(
                &cfg.ap_account,
                Direction::Debit,
                amount,
                &currency,
                Some(invoice.party_id.clone()),
            ),
            leg(
                &cfg.bank_account,
                Direction::Credit,
                amount,
                &currency,
                Some(invoice.party_id.clone()),
            ),
        ],
    };
    let entry = JournalEntry {
        as_of: Utc::now(),
        memo,
        legs,
        actor: actor.clone(),
    };
    entry.validate().map_err(InvoiceError::Journal)?;
    Ok(entry)
}

fn leg(
    account: &str,
    direction: Direction,
    amount: MinorAmount,
    currency: &klarbog_types::Currency,
    party_id: Option<klarbog_types::PartyId>,
) -> Leg {
    Leg {
        account: account.into(),
        direction,
        amount,
        currency: currency.clone(),
        party_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InvoiceId, InvoiceLine, InvoiceStatus};
    use klarbog_types::{Actor, Currency, PartyId};

    fn sample_invoice(kind: InvoiceKind) -> Invoice {
        Invoice {
            id: InvoiceId::new("inv_test"),
            party_id: PartyId::new("party_acme"),
            kind,
            lines: vec![InvoiceLine {
                description: "Widget".into(),
                amount_minor: 25_000,
                currency: Currency::new("DKK").unwrap(),
            }],
            status: InvoiceStatus::Draft,
            payments: Vec::new(),
            vat: None,
        }
    }

    fn vat_invoice(kind: InvoiceKind) -> Invoice {
        let mut inv = sample_invoice(kind);
        // 25000 net → 6250 vat → 31250 gross (business convention).
        inv.vat = Some(crate::InvoiceVat {
            net_minor: 25_000,
            vat_minor: 6_250,
            gross_minor: 31_250,
            rate_bps: 2_500,
        });
        inv
    }

    #[test]
    fn legacy_sale_books_two_legs_no_vat() {
        let inv = sample_invoice(InvoiceKind::Sale);
        let entry = journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert!(entry.legs.iter().all(|l| l.party_id.is_some()));
        assert_eq!(entry.legs.len(), 2);
        assert_eq!(entry.legs[0].account, "1100");
        assert_eq!(entry.legs[1].account, "1000");
        assert_eq!(entry.legs[0].amount.minor(), 25_000);
    }

    #[test]
    fn vat_sale_books_gross_ar_net_revenue_and_salgsmoms() {
        let inv = vat_invoice(InvoiceKind::Sale);
        let entry = journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert_eq!(entry.legs.len(), 3);
        assert_eq!(entry.legs[0].account, "1100");
        assert_eq!(entry.legs[0].amount.minor(), 31_250);
        assert_eq!(entry.legs[1].account, "1000");
        assert_eq!(entry.legs[1].amount.minor(), 25_000);
        assert_eq!(entry.legs[2].account, "1200");
        assert_eq!(entry.legs[2].amount.minor(), 6_250);
        assert!(entry.legs.iter().all(|l| l.party_id.is_some()));
    }

    #[test]
    fn vat_purchase_books_net_expense_koebsmoms_and_gross_ap() {
        let inv = vat_invoice(InvoiceKind::Purchase);
        let entry = journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert_eq!(entry.legs.len(), 3);
        assert_eq!(entry.legs[0].account, "3000");
        assert_eq!(entry.legs[0].amount.minor(), 25_000);
        assert_eq!(entry.legs[1].account, "7000");
        assert_eq!(entry.legs[1].amount.minor(), 31_250);
        assert_eq!(entry.legs[2].account, "4000");
        assert_eq!(entry.legs[2].amount.minor(), 6_250);
    }

    #[test]
    fn purchase_reverses_accounts() {
        let inv = sample_invoice(InvoiceKind::Purchase);
        let entry = journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert_eq!(entry.legs[0].account, "3000");
        assert_eq!(entry.legs[1].account, "7000");
    }

    #[test]
    fn credit_note_negates_send_booking_incl_vat() {
        let mut inv = vat_invoice(InvoiceKind::Sale);
        inv.status = InvoiceStatus::Sent;
        let entry = credit_journal_suggestion(
            &inv,
            "forkert beløb",
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap();
        assert_eq!(
            entry.memo,
            format!("invoice:{}:credit · forkert beløb", inv.id)
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
        let err =
            credit_journal_suggestion(&draft, "fejl", &Actor::user("t"), &InvoiceConfig::default())
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
        let err =
            credit_journal_suggestion(&inv, "  ", &Actor::user("t"), &InvoiceConfig::default())
                .unwrap_err();
        assert!(matches!(err, InvoiceError::MissingCreditReason));
    }

    #[test]
    fn vat_payment_settles_gross() {
        let inv = vat_invoice(InvoiceKind::Sale);
        let entry =
            payment_journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert_eq!(entry.legs[0].amount.minor(), 31_250);
        assert_eq!(inv.remaining_minor().unwrap(), 31_250);
    }

    #[test]
    fn payment_suggestion_has_party_on_legs() {
        let inv = sample_invoice(InvoiceKind::Sale);
        let entry =
            payment_journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert!(entry.legs.iter().all(|l| l.party_id.is_some()));
        assert_eq!(entry.legs[0].account, "2000");
        assert_eq!(entry.legs[1].account, "1100");
    }

    #[test]
    fn payment_suggestion_partial_amount() {
        let inv = sample_invoice(InvoiceKind::Sale);
        let entry = payment_journal_suggestion_amount(
            &inv,
            7_500,
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap();
        assert_eq!(entry.legs[0].amount.minor(), 7_500);
        assert!(entry.legs.iter().all(|l| l.party_id.is_some()));
    }
}
