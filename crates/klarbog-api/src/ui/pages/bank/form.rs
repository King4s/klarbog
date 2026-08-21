//! Bank form types and helpers.

use klarbog_plugin_bank::BankProfile;
use serde::Deserialize;

pub(super) struct DraftRow {
    pub idx: usize,
    pub date: String,
    pub memo: String,
    pub dkk: String,
    pub minor: String,
}

pub(super) struct SuggestRow {
    pub date: String,
    pub text: String,
    pub amount: String,
    pub best_invoice: String,
    pub confidence_bps: String,
    pub unsafe_reason: String,
    pub can_apply: bool,
}

#[derive(Deserialize, Default)]
pub struct BankActionForm {
    pub action: String,
    pub provider: String,
    pub csv: String,
    #[serde(default)]
    pub row_date: String,
    #[serde(default)]
    pub row_text: String,
    #[serde(default)]
    pub row_amount: String,
    #[serde(default)]
    pub invoice_id: String,
    #[serde(default)]
    pub force: String,
    #[serde(default)]
    pub row_index: String,
    #[serde(default)]
    pub entry_json: String,
    #[serde(default)]
    pub confirm_token: String,
    #[serde(default)]
    pub period_from: String,
    #[serde(default)]
    pub period_to: String,
}

pub(super) struct ReconReportRow {
    pub date: String,
    pub text: String,
    pub dkk: String,
    pub memo: String,
}

pub(super) fn parse_provider(raw: &str) -> BankProfile {
    match raw.trim() {
        "revolut" => BankProfile::Revolut,
        "stripe" => BankProfile::Stripe,
        _ => BankProfile::GenericDk,
    }
}

pub(super) fn provider_name(p: BankProfile) -> &'static str {
    match p {
        BankProfile::GenericDk => "generic_dk",
        BankProfile::Revolut => "revolut",
        BankProfile::Stripe => "stripe",
    }
}

pub(super) fn wants_force(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}
