//! Journal form + entry builders.

use chrono::Utc;
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_types::{Actor, Currency, MinorAmount};
use serde::Deserialize;

use super::super::common::ACTOR;

#[derive(Clone)]
pub(super) struct JournalFields {
    pub memo: String,
    pub account1: String,
    pub direction1: String,
    pub amount1: String,
    pub account2: String,
    pub direction2: String,
    pub amount2: String,
    pub moms_gross: String,
    pub moms_memo: String,
    pub has_moms: bool,
    pub moms_suggested: bool,
    pub moms_reason: String,
    pub moms_gross_minor: String,
    pub moms_net_minor: String,
    pub moms_vat_minor: String,
    pub moms_rate_bps: String,
    pub has_preview: bool,
    pub confirm_token: String,
    pub expires_unix_ms: String,
    pub payload_digest: String,
    pub entry_json: String,
}

impl Default for JournalFields {
    fn default() -> Self {
        Self {
            memo: "udgift #vat25 #receipt".into(),
            account1: "3000".into(),
            direction1: "debit".into(),
            amount1: "12500".into(),
            account2: "2000".into(),
            direction2: "credit".into(),
            amount2: "12500".into(),
            moms_gross: "12500".into(),
            moms_memo: "udgift #vat25 #receipt".into(),
            has_moms: false,
            moms_suggested: false,
            moms_reason: String::new(),
            moms_gross_minor: String::new(),
            moms_net_minor: String::new(),
            moms_vat_minor: String::new(),
            moms_rate_bps: String::new(),
            has_preview: false,
            confirm_token: String::new(),
            expires_unix_ms: String::new(),
            payload_digest: String::new(),
            entry_json: String::new(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct JournalActionForm {
    pub action: String,
    pub memo: String,
    pub account1: String,
    pub direction1: String,
    pub amount1: String,
    pub account2: String,
    pub direction2: String,
    pub amount2: String,
    pub moms_gross: String,
    pub moms_memo: String,
    pub confirm_token: String,
    pub entry_json: String,
    pub entry_id: String,
}

fn parse_direction(s: &str) -> Direction {
    match s.trim().to_ascii_lowercase().as_str() {
        "credit" => Direction::Credit,
        _ => Direction::Debit,
    }
}

pub(super) fn build_entry(form: &JournalActionForm) -> Result<JournalEntry, String> {
    let memo = form.memo.trim();
    if memo.is_empty() {
        return Err("Memo kræves".into());
    }
    let amount1: i64 = form
        .amount1
        .trim()
        .parse()
        .map_err(|_| "Ben 1 beløb skal være heltal (øre)".to_string())?;
    let amount2: i64 = form
        .amount2
        .trim()
        .parse()
        .map_err(|_| "Ben 2 beløb skal være heltal (øre)".to_string())?;
    let account1 = form.account1.trim();
    let account2 = form.account2.trim();
    if account1.is_empty() || account2.is_empty() {
        return Err("Begge konti kræves".into());
    }
    let currency = Currency::new("DKK").map_err(|e| e.to_string())?;
    Ok(JournalEntry {
        as_of: Utc::now(),
        memo: memo.to_string(),
        actor: Actor::user(ACTOR),
        legs: vec![
            Leg {
                account: account1.to_string(),
                direction: parse_direction(&form.direction1),
                amount: MinorAmount::from_minor(amount1),
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: account2.to_string(),
                direction: parse_direction(&form.direction2),
                amount: MinorAmount::from_minor(amount2),
                currency,
                party_id: None,
            },
        ],
    })
}

/// Købsmoms (indgående moms) — chart account for the VAT leg of a split.
pub(super) const VAT_PURCHASE_ACCOUNT: &str = "4000";

/// 3-leg VAT split: expense net debit, Købsmoms vat debit, credit gross.
pub(super) fn build_moms_split_entry(
    memo: &str,
    net: i64,
    vat: i64,
    gross: i64,
    expense_account: &str,
    credit_account: &str,
) -> Result<JournalEntry, String> {
    if memo.trim().is_empty() {
        return Err("Memo kræves".into());
    }
    let currency = Currency::new("DKK").map_err(|e| e.to_string())?;
    Ok(JournalEntry {
        as_of: Utc::now(),
        memo: memo.trim().to_string(),
        actor: Actor::user(ACTOR),
        legs: vec![
            Leg {
                account: expense_account.to_string(),
                direction: Direction::Debit,
                amount: MinorAmount::from_minor(net),
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: VAT_PURCHASE_ACCOUNT.to_string(),
                direction: Direction::Debit,
                amount: MinorAmount::from_minor(vat),
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: credit_account.to_string(),
                direction: Direction::Credit,
                amount: MinorAmount::from_minor(gross),
                currency,
                party_id: None,
            },
        ],
    })
}

fn or_default(value: &str, default: String) -> String {
    if value.trim().is_empty() {
        default
    } else {
        value.to_string()
    }
}

/// Form values win; empty fields (e.g. the moms forms only submit moms_*)
/// fall back to defaults so dropdowns keep a sensible selection.
pub(super) fn fields_from_form(form: &JournalActionForm) -> JournalFields {
    let d = JournalFields::default();
    JournalFields {
        memo: form.memo.clone(),
        account1: or_default(&form.account1, d.account1),
        direction1: or_default(&form.direction1, d.direction1),
        amount1: or_default(&form.amount1, d.amount1),
        account2: or_default(&form.account2, d.account2),
        direction2: or_default(&form.direction2, d.direction2),
        amount2: or_default(&form.amount2, d.amount2),
        moms_gross: form.moms_gross.clone(),
        moms_memo: form.moms_memo.clone(),
        ..Default::default()
    }
}
