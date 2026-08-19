//! Danish bookkeeping rules (DEV). Full SKAT/Moms later.

use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_plugin::{Capability, Plugin, RulesPlugin};
use klarbog_types::KlarbogError;

pub struct RulesDkPlugin;

impl Plugin for RulesDkPlugin {
    fn id(&self) -> &'static str {
        "rules-dk"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::RulesValidate]
    }
}

impl RulesPlugin for RulesDkPlugin {
    fn validate_entry(&self, entry: &JournalEntry) -> Result<Vec<String>, KlarbogError> {
        let mut applied = Vec::new();
        let mut errors = Vec::new();

        if entry.memo.trim().is_empty() {
            errors.push("memo required (dk.bookkeeping.memo)".into());
        }

        match parse_vat_rate_from_memo(&entry.memo) {
            Ok(Some(_)) => applied.push("dk.vat.rate".into()),
            Ok(None) => {}
            Err(msg) => errors.push(msg),
        }

        let has_receipt = has_receipt_signal(&entry.memo);

        for leg in &entry.legs {
            if let Err(msg) = validate_account(&leg.account) {
                errors.push(msg);
            }
            if leg.amount.minor() == 0 {
                errors.push(format!(
                    "zero amount on account {} (dk.bookkeeping.nonzero)",
                    leg.account
                ));
            }
            if is_expense_debit(leg) {
                let has_party = leg.party_id.is_some();
                if !has_party && !has_receipt {
                    // Fail-closed: expense debit needs party_id or memo receipt signal.
                    errors.push(format!(
                        "expense debit on {} requires party_id or receipt signal (#receipt / document_id:) (dk.expense.receipt_required)",
                        leg.account
                    ));
                } else if has_party && !has_receipt {
                    // Softer: party known but no receipt tag yet.
                    applied.push("dk.expense.receipt_hint".into());
                }
            }
        }

        if !errors.is_empty() {
            return Err(KlarbogError::Message(errors.join("; ")));
        }

        applied.sort();
        applied.dedup();
        Ok(applied)
    }
}

fn validate_account(account: &str) -> Result<(), String> {
    if account.trim().is_empty() {
        return Err("empty account (dk.bookkeeping.account_range)".into());
    }
    if !account.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "invalid account {account:?} (dk.bookkeeping.account_range)"
        ));
    }
    Ok(())
}

fn is_expense_account(account: &str) -> bool {
    account
        .parse::<u32>()
        .ok()
        .is_some_and(|n| (4000..=6999).contains(&n))
}

fn is_expense_debit(leg: &Leg) -> bool {
    leg.direction == Direction::Debit && leg.amount.minor() > 0 && is_expense_account(&leg.account)
}

/// Receipt / linked-document signal in memo (whitespace-separated tokens).
///
/// Accepted forms (case-insensitive):
/// - `#receipt` (exact token, or `#receipt:...`)
/// - `document_id:<id>` or `document_id=<id>`
fn has_receipt_signal(memo: &str) -> bool {
    let lower = memo.to_lowercase();
    for word in lower.split_whitespace() {
        if word == "#receipt" || word.starts_with("#receipt:") {
            return true;
        }
        if let Some(rest) = word.strip_prefix("document_id:") {
            if !rest.is_empty() {
                return true;
            }
        }
        if let Some(rest) = word.strip_prefix("document_id=") {
            if !rest.is_empty() {
                return true;
            }
        }
    }
    false
}

/// Parse optional VAT rate hints from memo/tags text (25 or 0 only; no money math).
fn parse_vat_rate_from_memo(memo: &str) -> Result<Option<i32>, String> {
    let lower = memo.to_lowercase();
    let mut found: Option<i32> = None;

    let mut note_rate = |rate: i32| -> Result<(), String> {
        if rate != 0 && rate != 25 {
            return Err(format!("unsupported VAT rate {rate} in memo (dk.vat.rate)"));
        }
        if let Some(prev) = found {
            if prev != rate {
                return Err("conflicting VAT rates in memo (dk.vat.rate)".into());
            }
        } else {
            found = Some(rate);
        }
        Ok(())
    };

    for word in lower.split_whitespace() {
        for sep in [':', '='] {
            for key in ["vat", "moms"] {
                let prefix = format!("{key}{sep}");
                if let Some(rest) = word.strip_prefix(&prefix) {
                    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if !digits.is_empty() {
                        if let Ok(rate) = digits.parse::<i32>() {
                            note_rate(rate)?;
                        }
                    }
                }
            }
        }
        if let Some(num) = word.strip_suffix('%') {
            if let Ok(rate) = num.parse::<i32>() {
                note_rate(rate)?;
            }
        }
    }

    for (tag, rate) in [("#vat25", 25), ("#moms25", 25), ("#vat0", 0), ("#moms0", 0)] {
        if lower.contains(tag) {
            note_rate(rate)?;
        }
    }

    Ok(found)
}

pub static RULES_DK: RulesDkPlugin = RulesDkPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_journal::{Direction, Leg};
    use klarbog_types::{Actor, Currency, MinorAmount, PartyId};

    fn entry(memo: &str, minor: i64) -> JournalEntry {
        entry_with_party(memo, minor, None)
    }

    fn entry_with_party(memo: &str, minor: i64, party_id: Option<PartyId>) -> JournalEntry {
        let amount = MinorAmount::from_minor(minor);
        let currency = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: memo.into(),
            actor: Actor::user("t"),
            legs: vec![
                Leg {
                    account: "6000".into(),
                    direction: Direction::Debit,
                    amount,
                    currency: currency.clone(),
                    party_id: party_id.clone(),
                },
                Leg {
                    account: "5800".into(),
                    direction: Direction::Credit,
                    amount,
                    currency,
                    party_id: None,
                },
            ],
        }
    }

    #[test]
    fn rejects_empty_memo() {
        let err = RULES_DK.validate_entry(&entry("", 100)).unwrap_err();
        assert!(err.to_string().contains("memo"));
    }

    #[test]
    fn rejects_zero_amount_leg() {
        let err = RULES_DK.validate_entry(&entry("ok", 0)).unwrap_err();
        assert!(err.to_string().contains("zero amount"));
    }

    #[test]
    fn rejects_invalid_account() {
        let mut e = entry("ok #receipt", 100);
        e.legs[0].account = "6abc".into();
        let err = RULES_DK.validate_entry(&e).unwrap_err();
        assert!(err.to_string().contains("account_range"));
    }

    #[test]
    fn rejects_empty_account() {
        let mut e = entry("ok #receipt", 100);
        e.legs[0].account = "   ".into();
        let err = RULES_DK.validate_entry(&e).unwrap_err();
        assert!(err.to_string().contains("account_range"));
    }

    #[test]
    fn rejects_unsupported_vat_rate() {
        let err = RULES_DK
            .validate_entry(&entry("office vat:12 #receipt", 100))
            .unwrap_err();
        assert!(err.to_string().contains("dk.vat.rate"));
    }

    #[test]
    fn flags_vat_rate_hint() {
        let applied = RULES_DK
            .validate_entry(&entry("supplies moms:25 #receipt", 500))
            .unwrap();
        assert!(applied.contains(&"dk.vat.rate".to_string()));
    }

    #[test]
    fn blocks_expense_without_party_or_receipt() {
        let err = RULES_DK
            .validate_entry(&entry("office supplies", 500))
            .unwrap_err();
        assert!(err.to_string().contains("dk.expense.receipt_required"));
    }

    #[test]
    fn allows_expense_with_receipt_tag() {
        let applied = RULES_DK
            .validate_entry(&entry("office supplies #receipt", 500))
            .unwrap();
        assert!(!applied.contains(&"dk.expense.receipt_hint".to_string()));
    }

    #[test]
    fn allows_expense_with_document_id_signal() {
        let applied = RULES_DK
            .validate_entry(&entry("office document_id:doc-9", 500))
            .unwrap();
        assert!(applied.is_empty());
    }

    #[test]
    fn hints_when_party_without_receipt() {
        let party = PartyId::new("vendor-1");
        let applied = RULES_DK
            .validate_entry(&entry_with_party("office supplies", 500, Some(party)))
            .unwrap();
        assert_eq!(applied, vec!["dk.expense.receipt_hint"]);
    }

    #[test]
    fn allows_party_plus_receipt_tag() {
        let party = PartyId::new("vendor-1");
        let applied = RULES_DK
            .validate_entry(&entry_with_party("office #receipt", 500, Some(party)))
            .unwrap();
        assert!(!applied.contains(&"dk.expense.receipt_hint".to_string()));
    }

    #[test]
    fn no_receipt_rule_without_expense_debit() {
        let amount = MinorAmount::from_minor(100);
        let currency = Currency::new("DKK").unwrap();
        let e = JournalEntry {
            as_of: Utc::now(),
            memo: "transfer".into(),
            actor: Actor::user("t"),
            legs: vec![
                Leg {
                    account: "1000".into(),
                    direction: Direction::Debit,
                    amount,
                    currency: currency.clone(),
                    party_id: None,
                },
                Leg {
                    account: "5800".into(),
                    direction: Direction::Credit,
                    amount,
                    currency,
                    party_id: None,
                },
            ],
        };
        let applied = RULES_DK.validate_entry(&e).unwrap();
        assert!(applied.is_empty());
    }
}
