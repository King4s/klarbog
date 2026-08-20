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
}

impl Default for JournalFields {
    fn default() -> Self {
        Self {
            memo: "udgift #vat25 #receipt".into(),
            account1: "6000".into(),
            direction1: "debit".into(),
            amount1: "12500".into(),
            account2: "5800".into(),
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
        }
    }
}

#[derive(Deserialize, Default)]
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

pub(super) fn fields_from_form(form: &JournalActionForm) -> JournalFields {
    JournalFields {
        memo: form.memo.clone(),
        account1: form.account1.clone(),
        direction1: form.direction1.clone(),
        amount1: form.amount1.clone(),
        account2: form.account2.clone(),
        direction2: form.direction2.clone(),
        amount2: form.amount2.clone(),
        moms_gross: form.moms_gross.clone(),
        moms_memo: form.moms_memo.clone(),
        ..Default::default()
    }
}
