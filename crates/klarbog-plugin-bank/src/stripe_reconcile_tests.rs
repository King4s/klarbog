//! Unit tests for Stripe consume → reconcile suggest / apply-preview pipeline.

use crate::config::StripeWebhookConfig;
use crate::reconcile::list_unmatched_bank_exceptions;
use crate::stripe_reconcile::{apply_preview_from_stripe_consume, suggest_from_stripe_consume};
use crate::webhook::{ingest_stripe_webhook, sign_test_payload};
use crate::webhook_consume::{ConsumeOpts, STRIPE_WEBHOOKS_CONSUMED};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::Actor;
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

#[test]
fn unique_safe_applies_journal_suggestion() {
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

    let actor = Actor::user("owner");
    let report =
        apply_preview_from_stripe_consume(&company, ConsumeOpts::default(), &actor, false).unwrap();
    assert!(report.consume.dry_run);
    let applied = report.applied.expect("unique safe apply");
    assert_eq!(applied.row_index, 0);
    assert!(applied.result.entry.memo.contains("bank:"));
    assert!(!applied.result.forced);
}

#[test]
fn no_safe_match_skips_apply() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    fs::create_dir_all(&company).unwrap();
    let party = upsert_party(&company, None, "Other Party".into()).unwrap();
    create_draft_from_new(
        &company,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Mismatch".into(),
            amount_minor: 99_999,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    ingest_charge(&company);

    let actor = Actor::user("owner");
    let report =
        apply_preview_from_stripe_consume(&company, ConsumeOpts::default(), &actor, false).unwrap();
    assert!(report.applied.is_none());
    assert_eq!(report.matches.len(), 1);
}

#[test]
fn unique_safe_apply_closes_prior_unmatched_exception() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    fs::create_dir_all(&company).unwrap();
    ingest_charge(&company);

    let raised = suggest_from_stripe_consume(&company, ConsumeOpts::default()).unwrap();
    assert_eq!(list_unmatched_bank_exceptions(&company).unwrap().len(), 1);
    assert!(!raised.exceptions_raised.is_empty());

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

    let actor = Actor::user("owner");
    let report =
        apply_preview_from_stripe_consume(&company, ConsumeOpts::default(), &actor, false).unwrap();
    let applied = report.applied.expect("unique safe apply");
    assert!(applied.result.exception_closed.is_some());
    assert!(report.exceptions.is_empty());
    assert!(list_unmatched_bank_exceptions(&company).unwrap().is_empty());
}
