//! Party-specific payment terms (DK-INVOICE-DUE-DATE-001 / EJER-3).

use crate::Party;

/// Effective payment terms: explicit party frist or company default.
pub fn resolve_payment_terms_days(party: &Party, company_default: u32) -> u32 {
    party.payment_terms_days.unwrap_or(company_default)
}

/// When the kundekort has an explicit frist that differs from company standard.
pub fn payment_terms_deviation_note(party: &Party, company_default: u32) -> Option<String> {
    match party.payment_terms_days {
        Some(customer_terms) if customer_terms != company_default => Some(format!(
            "Betalingsfrist {customer_terms} dage fra kundekortet — virksomhedens standard er {company_default} dage."
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PartyKind;
    use klarbog_types::PartyId;

    fn party(terms: Option<u32>) -> Party {
        Party {
            id: PartyId::new("party_x"),
            display_name: "Kunde".into(),
            kind: PartyKind::Private,
            payment_terms_days: terms,
        }
    }

    #[test]
    fn inherit_uses_company_default() {
        assert_eq!(resolve_payment_terms_days(&party(None), 30), 30);
    }

    #[test]
    fn explicit_party_terms_win() {
        assert_eq!(resolve_payment_terms_days(&party(Some(14)), 30), 14);
    }

    #[test]
    fn deviation_note_only_when_explicit_and_different() {
        assert!(payment_terms_deviation_note(&party(None), 30).is_none());
        assert!(payment_terms_deviation_note(&party(Some(30)), 30).is_none());
        let note = payment_terms_deviation_note(&party(Some(14)), 30).unwrap();
        assert!(note.contains("14 dage"));
        assert!(note.contains("30 dage"));
    }
}
