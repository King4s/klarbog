//! Credit note journal suggestion (ADR-020 / DK-CREDIT-NOTE-001).
//! Full and partial credits with cumulative loft against original gross —
//! proportional VAT allocation as in originalens `issueCreditNote`.

use crate::draft::InvoiceConfig;
use crate::{Invoice, InvoiceError, InvoiceKind, InvoiceStatus};
use chrono::Utc;
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_types::{Actor, MinorAmount};

/// Round half-up positive `(a * b) / c` in minor units (øre).
fn mul_div_round(a: i64, b: i64, c: i64) -> Result<i64, InvoiceError> {
    if c <= 0 {
        return Err(InvoiceError::Overflow);
    }
    let numer = a
        .checked_mul(b)
        .ok_or(InvoiceError::Overflow)?
        .checked_add(c / 2)
        .ok_or(InvoiceError::Overflow)?;
    Ok(numer / c)
}

/// Allocate this credit's net/vat/gross against prior credits on the invoice.
/// Final credit (cumulative == original gross) takes the residual so the
/// sum of all credits lands exactly on original net/vat/gross.
pub fn allocate_credit_amounts(
    invoice: &Invoice,
    gross_minor: i64,
) -> Result<(i64, i64, i64), InvoiceError> {
    let remaining = invoice.creditable_remaining_minor()?;
    if gross_minor <= 0 {
        return Err(InvoiceError::CreditAmountInvalid {
            amount_minor: gross_minor,
        });
    }
    if gross_minor > remaining {
        return Err(InvoiceError::CreditExceedsRemaining {
            amount_minor: gross_minor,
            remaining_minor: remaining,
        });
    }
    let orig_gross = invoice.gross_minor()?;
    let credited_gross = invoice.credited_gross_minor()?;
    let cumulative = credited_gross
        .checked_add(gross_minor)
        .ok_or(InvoiceError::Overflow)?;

    match &invoice.vat {
        None => Ok((gross_minor, 0, gross_minor)),
        Some(v) => {
            let credited_vat = invoice.credited_vat_minor()?;
            let credited_net = invoice.credited_net_minor()?;
            let (this_net, this_vat) = if cumulative == orig_gross {
                // Residual on final note — exact landing on original.
                (v.net_minor - credited_net, v.vat_minor - credited_vat)
            } else {
                let cum_vat = mul_div_round(v.vat_minor, cumulative, orig_gross)?;
                let this_vat = cum_vat - credited_vat;
                let this_net = gross_minor - this_vat;
                (this_net, this_vat)
            };
            if this_net < 0 || this_vat < 0 {
                return Err(InvoiceError::Overflow);
            }
            if this_net + this_vat != gross_minor {
                return Err(InvoiceError::Overflow);
            }
            Ok((this_net, this_vat, gross_minor))
        }
    }
}

fn credit_legs(
    invoice: &Invoice,
    net: i64,
    vat: i64,
    gross: i64,
    cfg: &InvoiceConfig,
) -> Result<Vec<Leg>, InvoiceError> {
    invoice.validate_lines()?;
    let currency = invoice.lines[0].currency.clone();
    let party = Some(invoice.party_id.clone());
    // Negation of the send booking: flip debit/credit vs journal_suggestion.
    let mut legs = match invoice.kind {
        InvoiceKind::Sale => vec![
            leg(
                &cfg.ar_account,
                Direction::Credit,
                MinorAmount::from_minor(gross),
                &currency,
                party.clone(),
            ),
            leg(
                &cfg.revenue_account,
                Direction::Debit,
                MinorAmount::from_minor(net),
                &currency,
                party.clone(),
            ),
        ],
        InvoiceKind::Purchase => vec![
            leg(
                &cfg.expense_account,
                Direction::Credit,
                MinorAmount::from_minor(net),
                &currency,
                party.clone(),
            ),
            leg(
                &cfg.ap_account,
                Direction::Debit,
                MinorAmount::from_minor(gross),
                &currency,
                party.clone(),
            ),
        ],
    };
    if vat > 0 {
        let (account, direction) = match invoice.kind {
            InvoiceKind::Sale => (&cfg.salgsmoms_account, Direction::Debit),
            InvoiceKind::Purchase => (&cfg.koebsmoms_account, Direction::Credit),
        };
        legs.push(leg(
            account,
            direction,
            MinorAmount::from_minor(vat),
            &currency,
            party,
        ));
    }
    Ok(legs)
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

/// Credit note suggestion. `gross_minor = None` credits the full remaining
/// creditable amount. Partial amounts must be > 0 and ≤ remaining
/// (`cumulative_credit_amount_lte_original_invoice`).
pub fn credit_journal_suggestion(
    invoice: &Invoice,
    credit_note_no: &str,
    reason: &str,
    gross_minor: Option<i64>,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<JournalEntry, InvoiceError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(InvoiceError::MissingCreditReason);
    }
    if !invoice.status.allows_credit() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: InvoiceStatus::Void,
        });
    }
    if !invoice.payments.is_empty() {
        return Err(InvoiceError::CreditWithPayments);
    }
    let remaining = invoice.creditable_remaining_minor()?;
    if remaining <= 0 {
        return Err(InvoiceError::NothingCreditable);
    }
    let amount = gross_minor.unwrap_or(remaining);
    let (net, vat, gross) = allocate_credit_amounts(invoice, amount)?;
    let legs = credit_legs(invoice, net, vat, gross, cfg)?;
    let entry = JournalEntry {
        as_of: Utc::now(),
        memo: format!("invoice:{}:credit:{credit_note_no} · {reason}", invoice.id),
        legs,
        actor: actor.clone(),
    };
    entry.validate().map_err(InvoiceError::Journal)?;
    Ok(entry)
}

/// Extract (net, vat, gross) from a credit journal entry (sale or purchase).
pub fn credit_amounts_from_entry(
    entry: &JournalEntry,
    cfg: &InvoiceConfig,
) -> Result<(i64, i64, i64), InvoiceError> {
    let mut net = 0i64;
    let mut vat = 0i64;
    let mut gross = 0i64;
    for leg in &entry.legs {
        let amt = leg.amount.minor();
        if leg.account == cfg.ar_account || leg.account == cfg.ap_account {
            gross = amt;
        } else if leg.account == cfg.revenue_account || leg.account == cfg.expense_account {
            net = amt;
        } else if leg.account == cfg.salgsmoms_account || leg.account == cfg.koebsmoms_account {
            vat = amt;
        }
    }
    if gross <= 0 {
        return Err(InvoiceError::CreditAmountInvalid {
            amount_minor: gross,
        });
    }
    if vat == 0 {
        net = gross;
    }
    if net + vat != gross {
        return Err(InvoiceError::Overflow);
    }
    Ok((net, vat, gross))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::vat_invoice;
    use crate::{InvoiceCredit, InvoiceKind};
    use klarbog_types::{Actor, Currency};

    #[test]
    fn full_credit_negates_send_booking_incl_vat() {
        let mut inv = vat_invoice(InvoiceKind::Sale);
        inv.status = InvoiceStatus::Sent;
        let entry = credit_journal_suggestion(
            &inv,
            "CN-2026-0001",
            "forkert beløb",
            None,
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap();
        assert_eq!(
            entry.memo,
            format!("invoice:{}:credit:CN-2026-0001 · forkert beløb", inv.id)
        );
        assert_eq!(entry.legs.len(), 3);
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
    fn partial_then_residual_lands_exactly_on_original() {
        let mut inv = vat_invoice(InvoiceKind::Sale);
        inv.status = InvoiceStatus::Sent;
        // First credit 10000 gross of 31250.
        let (n1, v1, g1) = allocate_credit_amounts(&inv, 10_000).unwrap();
        assert_eq!(g1, 10_000);
        assert_eq!(n1 + v1, 10_000);
        inv.credits.push(InvoiceCredit {
            unix_ms: 1,
            credit_note_no: "CN-2026-0001".into(),
            gross_minor: g1,
            net_minor: n1,
            vat_minor: v1,
        });
        // Second credit takes the rest — residual path.
        let remaining = inv.creditable_remaining_minor().unwrap();
        assert_eq!(remaining, 21_250);
        let (n2, v2, g2) = allocate_credit_amounts(&inv, remaining).unwrap();
        assert_eq!(g2, 21_250);
        assert_eq!(n1 + n2, 25_000);
        assert_eq!(v1 + v2, 6_250);
        assert_eq!(n2 + v2, g2);
    }

    #[test]
    fn cumulative_cap_rejects_over_credit() {
        let mut inv = vat_invoice(InvoiceKind::Sale);
        inv.status = InvoiceStatus::Sent;
        let err = allocate_credit_amounts(&inv, 40_000).unwrap_err();
        assert!(matches!(
            err,
            InvoiceError::CreditExceedsRemaining {
                amount_minor: 40_000,
                remaining_minor: 31_250
            }
        ));
    }

    #[test]
    fn credit_note_refuses_draft_and_paid_invoices() {
        let draft = vat_invoice(InvoiceKind::Sale);
        let err = credit_journal_suggestion(
            &draft,
            "CN-2026-0001",
            "fejl",
            None,
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
            None,
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
            None,
            &Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(err, InvoiceError::MissingCreditReason));
    }
}
