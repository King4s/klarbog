//! Slice 44: fail-closed multi-currency batch vs company DKK (CSV + API fixtures).

use crate::api::{parse_revolut_api_json, parse_stripe_api_json};
use crate::csv::BankCsvError;
use crate::import::{import_preview, BankImportError, BankImportSource};
use crate::map::BankImportConfig;
use crate::{parse_revolut_csv, parse_stripe_csv, BankProfile};
use klarbog_types::{Actor, Currency};

fn dkk() -> Currency {
    Currency::new("DKK").unwrap()
}

#[test]
fn revolut_csv_mixed_batch_vs_company_dkk() {
    let csv = "Completed Date,Description,Amount,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
    let err = parse_revolut_csv(csv, Some(&dkk())).unwrap_err();
    assert!(matches!(
        err,
        BankCsvError::MixedCurrency { ref found } if found.contains("DKK") && found.contains("EUR")
    ));
}

#[test]
fn stripe_csv_mixed_batch_vs_company_dkk() {
    let csv = "Created,Description,Net,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
    let err = parse_stripe_csv(csv, Some(&dkk())).unwrap_err();
    assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
}

#[test]
fn revolut_csv_eur_only_mismatches_company_dkk() {
    let csv = "Completed Date,Description,Amount,Currency\n2026-01-01,A,-10.00,EUR\n";
    let err = parse_revolut_csv(csv, Some(&dkk())).unwrap_err();
    assert!(matches!(
        err,
        BankCsvError::CurrencyMismatch {
            ref expected,
            ref found
        } if expected == "DKK" && found == "EUR"
    ));
}

#[test]
fn stripe_csv_eur_only_mismatches_company_dkk() {
    let csv = "Created,Description,Net,Currency\n2026-01-01,A,-10.00,EUR\n";
    let err = parse_stripe_csv(csv, Some(&dkk())).unwrap_err();
    assert!(matches!(
        err,
        BankCsvError::CurrencyMismatch {
            ref expected,
            ref found
        } if expected == "DKK" && found == "EUR"
    ));
}

#[test]
fn revolut_api_mixed_batch_vs_company_dkk() {
    let json = r#"[
      {"completed_at":"2026-01-01T00:00:00Z","description":"A","legs":[{"amount":"-10.00","currency":"DKK"}]},
      {"completed_at":"2026-01-02T00:00:00Z","description":"B","legs":[{"amount":"5.00","currency":"EUR"}]}
    ]"#;
    let err = parse_revolut_api_json(json, Some(&dkk())).unwrap_err();
    assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
}

#[test]
fn stripe_api_mixed_batch_vs_company_dkk() {
    let json = r#"{"data":[
      {"created":1735689600,"description":"A","net":-1000,"currency":"dkk"},
      {"created":1735776000,"description":"B","net":500,"currency":"eur"}
    ]}"#;
    let err = parse_stripe_api_json(json, Some(&dkk())).unwrap_err();
    assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
}

#[test]
fn revolut_api_eur_only_mismatches_company_dkk() {
    let json = r#"[{
      "completed_at":"2026-01-01T00:00:00Z","description":"A",
      "legs":[{"amount":"-10.00","currency":"EUR"}]
    }]"#;
    let err = parse_revolut_api_json(json, Some(&dkk())).unwrap_err();
    assert!(matches!(
        err,
        BankCsvError::CurrencyMismatch {
            ref expected,
            ref found
        } if expected == "DKK" && found == "EUR"
    ));
}

#[test]
fn stripe_api_eur_only_mismatches_company_dkk() {
    let json = r#"{"data":[
      {"created":1735689600,"description":"A","net":-1000,"currency":"eur"}
    ]}"#;
    let err = parse_stripe_api_json(json, Some(&dkk())).unwrap_err();
    assert!(matches!(
        err,
        BankCsvError::CurrencyMismatch {
            ref expected,
            ref found
        } if expected == "DKK" && found == "EUR"
    ));
}

#[tokio::test]
async fn import_preview_revolut_csv_mixed_vs_dkk_fails_closed() {
    let csv = "Completed Date,Description,Amount,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
    let cfg = BankImportConfig {
        currency: dkk(),
        ..BankImportConfig::default()
    };
    let err = import_preview(
        BankImportSource::Csv,
        BankProfile::Revolut,
        Some(csv),
        &cfg,
        &Actor::agent("t"),
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        BankImportError::Csv(BankCsvError::MixedCurrency { .. })
    ));
}

#[tokio::test]
async fn import_preview_stripe_csv_eur_vs_dkk_fails_closed() {
    let csv = "Created,Description,Net,Currency\n2026-01-01,A,-10.00,EUR\n";
    let cfg = BankImportConfig {
        currency: dkk(),
        ..BankImportConfig::default()
    };
    let err = import_preview(
        BankImportSource::Csv,
        BankProfile::Stripe,
        Some(csv),
        &cfg,
        &Actor::agent("t"),
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        BankImportError::Csv(BankCsvError::CurrencyMismatch { .. })
    ));
}

#[test]
fn matching_dkk_batch_keeps_i64_minor() {
    let csv = "Completed Date,Description,Amount,Currency\n2026-01-01,A,-10.50,DKK\n";
    let rows = parse_revolut_csv(csv, Some(&dkk())).unwrap();
    let minor: i64 = rows[0].amount_minor.minor();
    assert_eq!(minor, -1050_i64);
}
