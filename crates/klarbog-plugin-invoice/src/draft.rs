//! Balanced journal entry suggestions for invoice drafts (no posting).

use crate::{Invoice, InvoiceError, InvoiceKind};
use chrono::Utc;
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_types::{Actor, MinorAmount};

#[derive(Debug, Clone)]
pub struct InvoiceConfig {
    pub ar_account: String,
    pub revenue_account: String,
    pub ap_account: String,
    pub expense_account: String,
}

impl Default for InvoiceConfig {
    fn default() -> Self {
        Self {
            ar_account: "1500".into(),
            revenue_account: "6100".into(),
            ap_account: "4400".into(),
            expense_account: "6000".into(),
        }
    }
}

pub fn journal_suggestion(
    invoice: &Invoice,
    actor: &Actor,
    cfg: &InvoiceConfig,
) -> Result<JournalEntry, InvoiceError> {
    invoice.validate_lines()?;
    let total = invoice.total_minor()?;
    let currency = invoice.lines[0].currency.clone();
    let amount = MinorAmount::from_minor(total);
    let memo = format!("invoice:{}:{}", invoice.id, invoice.lines[0].description);
    let legs = match invoice.kind {
        InvoiceKind::Sale => vec![
            leg(
                &cfg.ar_account,
                Direction::Debit,
                amount,
                &currency,
                Some(invoice.party_id.clone()),
            ),
            leg(
                &cfg.revenue_account,
                Direction::Credit,
                amount,
                &currency,
                Some(invoice.party_id.clone()),
            ),
        ],
        InvoiceKind::Purchase => vec![
            leg(
                &cfg.expense_account,
                Direction::Debit,
                amount,
                &currency,
                Some(invoice.party_id.clone()),
            ),
            leg(
                &cfg.ap_account,
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
    use crate::{InvoiceId, InvoiceLine};
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
        }
    }

    #[test]
    fn sale_has_party_on_legs() {
        let inv = sample_invoice(InvoiceKind::Sale);
        let entry = journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert!(entry.legs.iter().all(|l| l.party_id.is_some()));
        assert_eq!(entry.legs[0].account, "1500");
        assert_eq!(entry.legs[1].account, "6100");
    }

    #[test]
    fn purchase_reverses_accounts() {
        let inv = sample_invoice(InvoiceKind::Purchase);
        let entry = journal_suggestion(&inv, &Actor::user("t"), &InvoiceConfig::default()).unwrap();
        assert_eq!(entry.legs[0].account, "6000");
        assert_eq!(entry.legs[1].account, "4400");
    }
}
