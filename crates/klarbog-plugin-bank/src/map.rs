//! Map parsed bank rows to balanced draft journal entries (no posting).

use crate::csv::BankRow;
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_types::{Actor, Currency, MinorAmount, MoneyError};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct BankImportConfig {
    pub expense_account: String,
    pub bank_account: String,
    pub income_account: String,
    pub currency: Currency,
}

impl Default for BankImportConfig {
    fn default() -> Self {
        Self {
            expense_account: "6000".into(),
            bank_account: "5800".into(),
            income_account: "6100".into(),
            currency: Currency::new("DKK").expect("DKK"),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankMapError {
    #[error("zero amount on row: {0}")]
    ZeroAmount(String),
    #[error(transparent)]
    Money(#[from] MoneyError),
}

pub fn draft_entries_from_rows(
    rows: &[BankRow],
    cfg: &BankImportConfig,
    actor: &Actor,
) -> Result<Vec<JournalEntry>, BankMapError> {
    rows.iter()
        .map(|row| row_to_draft(row, cfg, actor))
        .collect()
}

fn row_to_draft(
    row: &BankRow,
    cfg: &BankImportConfig,
    actor: &Actor,
) -> Result<JournalEntry, BankMapError> {
    let signed = row.amount_minor.minor();
    if signed == 0 {
        return Err(BankMapError::ZeroAmount(row.text.clone()));
    }
    let abs = MinorAmount::from_minor(signed.unsigned_abs() as i64);
    let memo = format!("bank:{}:{}", row.text, row.date.format("%Y-%m-%d"));
    let legs = if signed < 0 {
        vec![
            leg(&cfg.expense_account, Direction::Debit, abs, &cfg.currency),
            leg(&cfg.bank_account, Direction::Credit, abs, &cfg.currency),
        ]
    } else {
        vec![
            leg(&cfg.bank_account, Direction::Debit, abs, &cfg.currency),
            leg(&cfg.income_account, Direction::Credit, abs, &cfg.currency),
        ]
    };
    Ok(JournalEntry {
        as_of: row.date,
        memo,
        legs,
        actor: actor.clone(),
    })
}

fn leg(account: &str, direction: Direction, amount: MinorAmount, currency: &Currency) -> Leg {
    Leg {
        account: account.into(),
        direction,
        amount,
        currency: currency.clone(),
        party_id: None,
    }
}
