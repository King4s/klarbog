//! Danish bookkeeping rules (DEV). Full SKAT/Moms later.

mod chart;
mod moms_post_suggestion;
mod vat_split;

pub use chart::{
    chart_accounts, find_account, is_expense_account_code, is_known_dk_account,
    is_receipt_gated_expense_code, AccountType, ChartAccount, DK_CHART, DK_CHART_DEPRECIATION,
    DK_CHART_STAFF_MAX, DK_CHART_STAFF_MIN, RULE_KNOWN_ACCOUNT,
};
pub use moms_post_suggestion::{
    memo_requests_vat25, moms_post_suggestion, MomsPostSuggestion, MomsPostSuggestionError,
    MomsSuggestedLeg,
};
pub use vat_split::{
    split_vat25_inclusive, split_vat_from_net, split_vat_inclusive, VatSplitError,
    VatSplitSuggestion, BPS_PER_UNIT, DK_VAT25_INCLUSIVE_BPS, DK_VAT_STANDARD_BPS,
};

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
            Ok(Some(rate)) => {
                applied.push("dk.vat.rate".into());
                // Hint only: agents may call split_vat*_inclusive for leg amounts.
                if rate == 25 {
                    applied.push("dk.vat.split_hint".into());
                }
            }
            Ok(None) => {}
            Err(msg) => errors.push(msg),
        }

        let has_receipt = has_receipt_signal(&entry.memo);

        for leg in &entry.legs {
            match validate_account(&leg.account) {
                Err(msg) => errors.push(msg),
                Ok(()) if !is_known_dk_account(&leg.account) => {
                    // Hint only — empty / non-digit already hard-fail above.
                    applied.push(RULE_KNOWN_ACCOUNT.into());
                }
                Ok(()) => {}
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

fn is_receipt_gated_expense(account: &str) -> bool {
    account
        .parse::<i64>()
        .ok()
        .is_some_and(is_receipt_gated_expense_code)
}

fn is_expense_debit(leg: &Leg) -> bool {
    leg.direction == Direction::Debit
        && leg.amount.minor() > 0
        && is_receipt_gated_expense(&leg.account)
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
pub(crate) fn parse_vat_rate_from_memo(memo: &str) -> Result<Option<i64>, String> {
    let lower = memo.to_lowercase();
    let mut found: Option<i64> = None;

    let mut note_rate = |rate: i64| -> Result<(), String> {
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
                        if let Ok(rate) = digits.parse::<i64>() {
                            note_rate(rate)?;
                        }
                    }
                }
            }
        }
        if let Some(num) = word.strip_suffix('%') {
            if let Ok(rate) = num.parse::<i64>() {
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
#[path = "rules_tests.rs"]
mod tests;
