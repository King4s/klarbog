//! Unified bank import entry — CSV fallback or live API (ADR-008/009).

use crate::api::{fetch_revolut_transactions, fetch_stripe_balance_transactions, BankApiError};
use crate::config::{BankApiConfigError, RevolutApiConfig, StripeApiConfig};
use crate::csv::{parse_bank_csv_with_profile, BankCsvError, BankProfile};
use crate::map::{draft_entries_from_rows, BankImportConfig, BankMapError};
use klarbog_journal::JournalEntry;
use klarbog_types::Actor;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BankImportSource {
    Csv,
    Api,
}

#[derive(Debug, Error)]
pub enum BankImportError {
    #[error("missing csv body")]
    MissingCsv,
    #[error("api import not supported for profile {rail:?}")]
    ApiNotSupported { rail: BankProfile },
    #[error(transparent)]
    Csv(#[from] BankCsvError),
    #[error(transparent)]
    Api(#[from] BankApiError),
    #[error(transparent)]
    Config(#[from] BankApiConfigError),
    #[error(transparent)]
    Map(#[from] BankMapError),
}

pub fn default_source_for_rail(rail: BankProfile) -> BankImportSource {
    match rail {
        BankProfile::GenericDk => BankImportSource::Csv,
        BankProfile::Revolut | BankProfile::Stripe => BankImportSource::Api,
    }
}

pub async fn import_preview(
    source: BankImportSource,
    rail: BankProfile,
    csv: Option<&str>,
    cfg: &BankImportConfig,
    actor: &Actor,
    company: Option<&Path>,
) -> Result<(Vec<crate::csv::BankRow>, Vec<JournalEntry>), BankImportError> {
    let required = match rail {
        BankProfile::Revolut | BankProfile::Stripe => Some(&cfg.currency),
        BankProfile::GenericDk => None,
    };

    let rows = match (source, rail) {
        (BankImportSource::Csv, BankProfile::GenericDk) => {
            let csv = csv.ok_or(BankImportError::MissingCsv)?;
            parse_bank_csv_with_profile(rail, csv, required)?
        }
        (BankImportSource::Csv, BankProfile::Revolut | BankProfile::Stripe) => {
            let csv = csv.ok_or(BankImportError::MissingCsv)?;
            parse_bank_csv_with_profile(rail, csv, required)?
        }
        (BankImportSource::Api, BankProfile::Revolut) => {
            let api_cfg = match company {
                Some(path) => RevolutApiConfig::from_env_or_company_secrets(path)?,
                None => RevolutApiConfig::from_env()?,
            };
            fetch_revolut_transactions(&api_cfg, None, required).await?
        }
        (BankImportSource::Api, BankProfile::Stripe) => {
            let api_cfg = StripeApiConfig::from_env()?;
            fetch_stripe_balance_transactions(&api_cfg, None, required).await?
        }
        (BankImportSource::Api, BankProfile::GenericDk) => {
            return Err(BankImportError::ApiNotSupported { rail });
        }
    };

    let drafts = draft_entries_from_rows(&rows, cfg, actor)?;
    Ok((rows, drafts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::parse_revolut_api_json;

    const REVOLUT_JSON: &str = include_str!("../tests/fixtures/revolut_api.json");

    #[test]
    fn default_source_revolut_is_api() {
        assert_eq!(
            default_source_for_rail(BankProfile::Revolut),
            BankImportSource::Api
        );
    }

    #[test]
    fn default_source_generic_is_csv() {
        assert_eq!(
            default_source_for_rail(BankProfile::GenericDk),
            BankImportSource::Csv
        );
    }

    #[tokio::test]
    async fn csv_revolut_offline_fallback() {
        const CSV: &str = include_str!("../tests/fixtures/revolut_statement.csv");
        let cfg = BankImportConfig::default();
        let drafts = import_preview(
            BankImportSource::Csv,
            BankProfile::Revolut,
            Some(CSV),
            &cfg,
            &Actor::agent("t"),
            None,
        )
        .await
        .unwrap();
        assert_eq!(drafts.1.len(), 3);
    }

    #[tokio::test]
    async fn api_revolut_requires_env() {
        let _guard = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
        let cfg = BankImportConfig::default();
        let err = import_preview(
            BankImportSource::Api,
            BankProfile::Revolut,
            None,
            &cfg,
            &Actor::agent("t"),
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, BankImportError::Config(_)));
    }

    #[tokio::test]
    async fn api_revolut_uses_company_stored_token_when_env_missing() {
        let dir = tempfile::tempdir().unwrap();
        let company = dir.path().join("co");
        std::fs::create_dir_all(&company).unwrap();
        let tokens = crate::oauth::RevolutStoredTokens {
            access_token: "stored-access".into(),
            refresh_token: None,
            expires_at: None,
            token_type: None,
        };
        crate::oauth::save_revolut_tokens(&company, &tokens).unwrap();
        let _guard = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
        let api_cfg =
            RevolutApiConfig::from_env_or_company_secrets(&company).expect("stored token");
        assert_eq!(api_cfg.token, "stored-access");
    }

    #[test]
    fn parse_json_smoke() {
        let rows = parse_revolut_api_json(REVOLUT_JSON, Some(&cfg_currency())).unwrap();
        assert_eq!(rows.len(), 3);
    }

    fn cfg_currency() -> klarbog_types::Currency {
        klarbog_types::Currency::new("DKK").unwrap()
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            unsafe { std::env::remove_var(key) };
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }
}
