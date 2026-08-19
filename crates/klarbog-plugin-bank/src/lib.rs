//! Bank import plugin — API-primary for Revolut/Stripe; CSV fallback (ADR-006/008/009).
//! Read-only: never posts to the ledger.

mod amount;
mod api;
mod config;
mod csv;
mod import;
mod map;
mod oauth;
mod reconcile;
mod reconcile_apply;
#[cfg(test)]
mod reconcile_tests;
mod revolut;
mod stripe;
mod stripe_reconcile;
#[cfg(test)]
mod stripe_reconcile_tests;
mod webhook;
mod webhook_consume;
#[cfg(test)]
mod webhook_consume_tests;
#[cfg(test)]
mod webhook_tests;

pub use amount::{parse_amount_minor, BankAmountError};
pub use api::{
    fetch_revolut_transactions, fetch_stripe_balance_transactions, parse_revolut_api_json,
    parse_stripe_api_json, BankApiError, HttpClient, ReqwestHttpClient,
};
pub use config::{
    BankApiConfigError, RevolutApiConfig, RevolutOAuthConfig, StripeApiConfig, StripeWebhookConfig,
};
pub use csv::{
    parse_bank_csv, parse_bank_csv_with_profile, parse_revolut_csv, parse_stripe_csv, BankCsvError,
    BankProfile, BankRow,
};
pub use import::{default_source_for_rail, import_preview, BankImportError, BankImportSource};
pub use map::{draft_entries_from_rows, BankImportConfig, BankMapError};
pub use oauth::{
    from_env_or_company_secrets_refreshed, load_revolut_tokens, oauth_exchange_code,
    oauth_exchange_code_with_client, oauth_start, oauth_start_with_state, refresh_access_token,
    refresh_access_token_with_client, save_revolut_tokens, RevolutOAuthError, RevolutOAuthStart,
    RevolutStoredTokens,
};
pub use reconcile::{
    list_unmatched_bank_exceptions, suggest_matches, sync_unmatched_exceptions, BankRowMatchResult,
    MatchKind, MatchSuggestion, ReconcileError, EXCEPTION_UNMATCHED_BANK, SAFE_THRESHOLD_BPS,
};
pub use reconcile_apply::{apply_match, ApplyMatchResult};
pub use stripe_reconcile::{
    apply_preview_from_stripe_consume, apply_preview_from_stripe_consume_with,
    suggest_from_stripe_consume, suggest_from_stripe_consume_with, StripeReconcileApplied,
    StripeReconcileApplyPreviewReport, StripeReconcilePipelineError, StripeReconcileSuggestReport,
};
pub use webhook::{
    draft_from_event, draft_to_bank_row, ingest_stripe_webhook, parse_webhook_event,
    queue_stripe_webhook, sign_test_payload, verify_stripe_signature, QueuedStripeWebhook,
    StripeWebhookDraft, StripeWebhookError, STRIPE_WEBHOOKS_DIR, STRIPE_WEBHOOKS_QUEUE,
};
pub use webhook_consume::{
    consume_stripe_webhook_queue, ConsumeOpts, ConsumeReport, ConsumedBankRow,
    STRIPE_WEBHOOKS_CONSUMED,
};

use klarbog_plugin::{Capability, Plugin};

pub struct BankPlugin;

impl Plugin for BankPlugin {
    fn id(&self) -> &'static str {
        "bank"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::Read]
    }
}

pub static BANK: BankPlugin = BankPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use klarbog_types::{Actor, MinorAmount};

    const FIXTURE: &str = include_str!("../tests/fixtures/danish_bank.csv");
    const REVOLUT_FIXTURE: &str = include_str!("../tests/fixtures/revolut_statement.csv");
    const STRIPE_FIXTURE: &str = include_str!("../tests/fixtures/stripe_balance.csv");

    #[test]
    fn plugin_is_read_only() {
        assert!(!BANK.has_journal_write());
    }

    #[test]
    fn fixture_parses_and_maps_to_balanced_drafts() {
        let rows = parse_bank_csv(FIXTURE).expect("fixture csv");
        assert_eq!(rows.len(), 3);

        let cfg = BankImportConfig::default();
        let actor = Actor::agent("bank-import");
        let drafts = draft_entries_from_rows(&rows, &cfg, &actor).expect("map rows");
        assert_eq!(drafts.len(), 3);

        for entry in &drafts {
            entry.validate().expect("balanced draft");
            assert!(entry.memo.contains("bank:"));
        }

        let expense = &drafts[0];
        assert_eq!(expense.legs[0].account, "6000");
        assert_eq!(expense.legs[0].amount.minor(), 12550);
        assert_eq!(expense.legs[1].account, "5800");
    }

    #[test]
    fn decimal_half_even_from_csv() {
        let rows = parse_bank_csv("date;text;amount\n2026-01-02;round;-10,005\n").unwrap();
        let drafts =
            draft_entries_from_rows(&rows, &BankImportConfig::default(), &Actor::agent("t"))
                .unwrap();
        assert_eq!(drafts[0].legs[0].amount.minor(), 1000);
    }

    #[test]
    fn integer_ore_column() {
        let rows = parse_bank_csv("date;text;amount\n2026-01-03;ore;-250\n").unwrap();
        let drafts =
            draft_entries_from_rows(&rows, &BankImportConfig::default(), &Actor::agent("t"))
                .unwrap();
        assert_eq!(drafts[0].legs[0].amount.minor(), 250);
    }

    #[test]
    fn revolut_fixture_parses_and_maps() {
        let cfg = BankImportConfig::default();
        let rows = parse_revolut_csv(REVOLUT_FIXTURE, Some(&cfg.currency)).expect("revolut csv");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].amount_minor.minor(), -4550);
        assert_eq!(rows[2].text, "Transfer, internal");

        let drafts =
            draft_entries_from_rows(&rows, &cfg, &Actor::agent("revolut-import")).expect("map");
        assert_eq!(drafts.len(), 3);
        for entry in &drafts {
            entry.validate().expect("balanced draft");
        }
    }

    #[test]
    fn revolut_rejects_mixed_currency() {
        let csv = "Completed Date,Description,Amount,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
        let err = parse_revolut_csv(csv, None).unwrap_err();
        assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
    }

    #[test]
    fn revolut_rejects_currency_mismatch_with_config() {
        let csv = "Completed Date,Description,Amount,Currency\n2026-01-01,A,-10.00,EUR\n";
        let cfg = BankImportConfig::default();
        let err = parse_revolut_csv(csv, Some(&cfg.currency)).unwrap_err();
        assert!(matches!(err, BankCsvError::CurrencyMismatch { .. }));
    }

    #[test]
    fn stripe_fixture_parses_and_maps() {
        let cfg = BankImportConfig::default();
        let rows = parse_stripe_csv(STRIPE_FIXTURE, Some(&cfg.currency)).expect("stripe csv");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].text, "Stripe payout");
        assert_eq!(rows[0].amount_minor.minor(), -100000);
        assert_eq!(rows[1].amount_minor.minor(), 24275);

        let drafts =
            draft_entries_from_rows(&rows, &cfg, &Actor::agent("stripe-import")).expect("map");
        assert_eq!(drafts.len(), 3);
        for entry in &drafts {
            entry.validate().expect("balanced draft");
            assert!(entry.memo.contains("bank:"));
        }
    }

    #[test]
    fn stripe_rejects_mixed_currency() {
        let csv = "Created,Description,Net,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
        let err = parse_stripe_csv(csv, None).unwrap_err();
        assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
    }

    #[test]
    fn profile_dispatch_stripe() {
        let cfg = BankImportConfig::default();
        let rows =
            parse_bank_csv_with_profile(BankProfile::Stripe, STRIPE_FIXTURE, Some(&cfg.currency))
                .expect("stripe profile");
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn profile_dispatch_keeps_generic_dk() {
        let rows =
            parse_bank_csv_with_profile(BankProfile::GenericDk, FIXTURE, None).expect("generic");
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn rejects_unbalanced_internal_state() {
        let row = BankRow {
            date: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            text: "x".into(),
            amount_minor: MinorAmount::from_minor(0),
        };
        let err = draft_entries_from_rows(&[row], &BankImportConfig::default(), &Actor::agent("t"))
            .unwrap_err();
        assert!(matches!(err, BankMapError::ZeroAmount(_)));
    }

    #[test]
    fn bank_import_source_snake_case() {
        let v = serde_json::to_value(BankImportSource::Api).unwrap();
        assert_eq!(v, "api");
    }
}
