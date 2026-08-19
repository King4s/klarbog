use chrono::{DateTime, Utc};
use klarbog_types::{Actor, Currency, MinorAmount, PartyId};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::hash::entry_digest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Debit,
    Credit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leg {
    pub account: String,
    pub direction: Direction,
    pub amount: MinorAmount,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub party_id: Option<PartyId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub as_of: DateTime<Utc>,
    pub memo: String,
    pub legs: Vec<Leg>,
    pub actor: Actor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostedEntry {
    pub id: Uuid,
    pub entry: JournalEntry,
    pub digest: String,
    pub prev_digest: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JournalError {
    #[error("journal entry has no legs")]
    Empty,
    #[error("journal entry has a single leg")]
    SingleLeg,
    #[error("unbalanced: debit {debit} != credit {credit}")]
    Unbalanced { debit: i64, credit: i64 },
    #[error("mixed currencies in one entry")]
    MixedCurrency,
    #[error("overflow")]
    Overflow,
}

impl JournalEntry {
    /// Validate mandatory double-entry invariants.
    pub fn validate(&self) -> Result<(), JournalError> {
        match self.legs.len() {
            0 => return Err(JournalError::Empty),
            1 => return Err(JournalError::SingleLeg),
            _ => {}
        }
        let currency = &self.legs[0].currency;
        let mut debit: i64 = 0;
        let mut credit: i64 = 0;
        for leg in &self.legs {
            if &leg.currency != currency {
                return Err(JournalError::MixedCurrency);
            }
            match leg.direction {
                Direction::Debit => {
                    debit = debit
                        .checked_add(leg.amount.minor())
                        .ok_or(JournalError::Overflow)?;
                }
                Direction::Credit => {
                    credit = credit
                        .checked_add(leg.amount.minor())
                        .ok_or(JournalError::Overflow)?;
                }
            }
        }
        if debit != credit {
            return Err(JournalError::Unbalanced { debit, credit });
        }
        Ok(())
    }

    pub fn post(self, prev_digest: Option<String>) -> Result<PostedEntry, JournalError> {
        self.validate()?;
        let id = Uuid::new_v4();
        let digest = entry_digest(&id, &self, prev_digest.as_deref());
        Ok(PostedEntry {
            id,
            entry: self,
            digest,
            prev_digest,
        })
    }

    /// Build a reversing entry that exact-negates amounts (same accounts, flipped dirs).
    pub fn reversal(&self, as_of: DateTime<Utc>, actor: Actor, reason: impl Into<String>) -> Self {
        let legs = self
            .legs
            .iter()
            .map(|leg| Leg {
                account: leg.account.clone(),
                direction: match leg.direction {
                    Direction::Debit => Direction::Credit,
                    Direction::Credit => Direction::Debit,
                },
                amount: leg.amount,
                currency: leg.currency.clone(),
                party_id: leg.party_id.clone(),
            })
            .collect();
        Self {
            as_of,
            memo: reason.into(),
            legs,
            actor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_types::Currency;

    fn dkk(n: i64) -> (MinorAmount, Currency) {
        (MinorAmount::from_minor(n), Currency::new("DKK").unwrap())
    }

    #[test]
    fn rejects_unbalanced() {
        let (a, c) = dkk(100);
        let e = JournalEntry {
            as_of: Utc::now(),
            memo: "bad".into(),
            actor: Actor::user("t"),
            legs: vec![
                Leg {
                    account: "6000".into(),
                    direction: Direction::Debit,
                    amount: a,
                    currency: c.clone(),
                    party_id: None,
                },
                Leg {
                    account: "5800".into(),
                    direction: Direction::Credit,
                    amount: MinorAmount::from_minor(50),
                    currency: c,
                    party_id: None,
                },
            ],
        };
        assert!(matches!(e.validate(), Err(JournalError::Unbalanced { .. })));
    }

    #[test]
    fn rejects_empty_and_single() {
        let e = JournalEntry {
            as_of: Utc::now(),
            memo: "x".into(),
            actor: Actor::user("t"),
            legs: vec![],
        };
        assert_eq!(e.validate(), Err(JournalError::Empty));
        let (a, c) = dkk(1);
        let e2 = JournalEntry {
            as_of: Utc::now(),
            memo: "x".into(),
            actor: Actor::user("t"),
            legs: vec![Leg {
                account: "1".into(),
                direction: Direction::Debit,
                amount: a,
                currency: c,
                party_id: None,
            }],
        };
        assert_eq!(e2.validate(), Err(JournalError::SingleLeg));
    }

    #[test]
    fn accepts_balanced_and_reverses() {
        let (a, c) = dkk(2500);
        let e = JournalEntry {
            as_of: Utc::now(),
            memo: "expense".into(),
            actor: Actor::agent("bot"),
            legs: vec![
                Leg {
                    account: "6000".into(),
                    direction: Direction::Debit,
                    amount: a,
                    currency: c.clone(),
                    party_id: None,
                },
                Leg {
                    account: "5800".into(),
                    direction: Direction::Credit,
                    amount: a,
                    currency: c,
                    party_id: None,
                },
            ],
        };
        let posted = e.clone().post(None).unwrap();
        let rev = posted
            .entry
            .reversal(Utc::now(), Actor::user("t"), "fix")
            .post(Some(posted.digest.clone()))
            .unwrap();
        assert!(rev.entry.validate().is_ok());
    }
}
