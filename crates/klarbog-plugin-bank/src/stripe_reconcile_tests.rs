//! Unit tests for Stripe consume → reconcile suggest pipeline.

use crate::config::StripeWebhookConfig;
use crate::stripe_reconcile::suggest_from_stripe_consume;
use crate::webhook::{ingest_stripe_webhook, sign_test_payload};
use crate::webhook_consume::{ConsumeOpts, STRIPE_WEBHOOKS_CONSUMED};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use std::fs;
use tempfile::tempdir;

const CHARGE_FIXTURE: &str = include_str!("../tests/fixtures/stripe_webhook_charge_succeeded.json");
const TEST_SECRET: &str = "whsec_test_fixture_secret";

fn ingest_charge(company: &std::path::Path) {
    let cfg = StripeWebhookConfig {
        webhook_secret: TEST_SECRET.into(),
    };
    let ts = chrono::Utc::now().timestamp();
    let sig = sign_test_payload(CHARGE_FIXTURE.as_bytes(), TEST_SECRET, ts);
    ingest_stripe_webhook(company, CHARGE_FIXTURE.as_bytes(), Some(&sig), &cfg).unwrap();
}

#[test]
fn dry_run_pipeline_matches_open_sale() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    fs::create_dir_all(&company).unwrap();
    let party = upsert_party(&company, None, "Customer payment".into()).unwrap();
    create_draft_from_new(
        &company,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: 24_275,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    ingest_charge(&company);

    let report = suggest_from_stripe_consume(&company, ConsumeOpts::default()).unwrap();
    assert!(report.consume.dry_run);
    assert_eq!(report.consume.consumed_count, 1);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].amount_minor, 24_275);
    assert!(!report.matches[0].suggestions.is_empty());
    assert!(!company.join(STRIPE_WEBHOOKS_CONSUMED).exists());
}

#[test]
fn confirm_persists_consumed_then_suggests() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    fs::create_dir_all(&company).unwrap();
    ingest_charge(&company);

    let report = suggest_from_stripe_consume(
        &company,
        ConsumeOpts {
            dry_run: false,
            limit: None,
        },
    )
    .unwrap();
    assert!(!report.consume.dry_run);
    assert_eq!(report.consume.consumed_count, 1);
    assert!(company.join(STRIPE_WEBHOOKS_CONSUMED).exists());

    let again = suggest_from_stripe_consume(
        &company,
        ConsumeOpts {
            dry_run: false,
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(again.consume.consumed_count, 0);
    assert!(again.matches.is_empty());
}
