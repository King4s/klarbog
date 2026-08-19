//! Danish bookkeeping rule stubs (DEV). Full SKAT/Moms later.

use klarbog_journal::{Direction, JournalEntry};
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

        for leg in &entry.legs {
            if leg.amount.minor() == 0 {
                errors.push(format!(
                    "zero amount on account {} (dk.bookkeeping.nonzero)",
                    leg.account
                ));
            }
            if leg.account == "6000" && leg.direction == Direction::Debit && leg.amount.minor() > 0
            {
                applied.push("dk.expense.hint".into());
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

pub static RULES_DK: RulesDkPlugin = RulesDkPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_journal::{Direction, Leg};
    use klarbog_types::{Actor, Currency, MinorAmount};

    fn entry(memo: &str, minor: i64) -> JournalEntry {
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
    fn flags_expense_hint() {
        let applied = RULES_DK
            .validate_entry(&entry("office supplies", 500))
            .unwrap();
        assert_eq!(applied, vec!["dk.expense.hint"]);
    }

    #[test]
    fn no_hint_without_6000_debit() {
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
