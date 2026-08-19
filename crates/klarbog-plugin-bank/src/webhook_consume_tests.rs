//! Unit tests for Stripe webhook queue consumer (ADR-009).

use crate::config::StripeWebhookConfig;
use crate::webhook::{
    ingest_stripe_webhook, queue_stripe_webhook, sign_test_payload, QueuedStripeWebhook,
    StripeWebhookDraft,
};
use crate::webhook_consume::{consume_stripe_webhook_queue, ConsumeOpts, STRIPE_WEBHOOKS_CONSUMED};
use chrono::Utc;
use std::path::Path;
use tempfile::tempdir;

const PAYOUT_FIXTURE: &str = include_str!("../tests/fixtures/stripe_webhook_payout_paid.json");
const CHARGE_FIXTURE: &str = include_str!("../tests/fixtures/stripe_webhook_charge_succeeded.json");
const TEST_SECRET: &str = "whsec_test_fixture_secret";

fn queue_both(company: &Path) {
    let cfg = StripeWebhookConfig {
        webhook_secret: TEST_SECRET.into(),
    };
    let ts = Utc::now().timestamp();
    for fixture in [PAYOUT_FIXTURE, CHARGE_FIXTURE] {
        let sig = sign_test_payload(fixture.as_bytes(), TEST_SECRET, ts);
        ingest_stripe_webhook(company, fixture.as_bytes(), Some(&sig), &cfg).unwrap();
    }
}

#[test]
fn dry_run_emits_rows_without_sidecar() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    queue_both(&company);

    let report = consume_stripe_webhook_queue(&company, ConsumeOpts::default()).unwrap();
    assert!(report.dry_run);
    assert_eq!(report.consumed_count, 2);
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.rows[0].amount_minor, -100_000);
    assert_eq!(report.rows[1].amount_minor, 24_275);
    assert!(!company.join(STRIPE_WEBHOOKS_CONSUMED).exists());
    assert_eq!(report.rows[0].to_bank_row().amount_minor.minor(), -100_000);
}

#[test]
fn confirm_is_idempotent_by_event_id() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    queue_both(&company);

    let first = consume_stripe_webhook_queue(
        &company,
        ConsumeOpts {
            dry_run: false,
            limit: None,
        },
    )
    .unwrap();
    assert!(!first.dry_run);
    assert_eq!(first.consumed_count, 2);
    assert!(company.join(STRIPE_WEBHOOKS_CONSUMED).exists());

    let second = consume_stripe_webhook_queue(
        &company,
        ConsumeOpts {
            dry_run: false,
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(second.consumed_count, 0);
    assert_eq!(second.skipped_already, 2);
    assert!(second.rows.is_empty());
}

#[test]
fn limit_caps_new_rows() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    queue_both(&company);

    let report = consume_stripe_webhook_queue(
        &company,
        ConsumeOpts {
            dry_run: true,
            limit: Some(1),
        },
    )
    .unwrap();
    assert_eq!(report.consumed_count, 1);
    assert_eq!(report.rows[0].event_type, "payout.paid");
}

#[test]
fn duplicate_event_id_in_queue_consumed_once() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    let entry = QueuedStripeWebhook {
        event_id: "evt_dup".into(),
        event_type: "payout.paid".into(),
        object_id: "po_1".into(),
        received_at: Utc::now(),
        draft: Some(StripeWebhookDraft {
            date: Utc::now(),
            text: "Stripe payout po_1".into(),
            amount_minor: -500,
            currency: "DKK".into(),
        }),
    };
    queue_stripe_webhook(&company, &entry).unwrap();
    queue_stripe_webhook(&company, &entry).unwrap();

    let report = consume_stripe_webhook_queue(
        &company,
        ConsumeOpts {
            dry_run: false,
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(report.consumed_count, 1);
    assert_eq!(report.skipped_already, 1);
}

#[test]
fn no_draft_skipped_but_marked_on_confirm() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    let entry = QueuedStripeWebhook {
        event_id: "evt_nodraft".into(),
        event_type: "customer.created".into(),
        object_id: "cus_1".into(),
        received_at: Utc::now(),
        draft: None,
    };
    queue_stripe_webhook(&company, &entry).unwrap();

    let report = consume_stripe_webhook_queue(
        &company,
        ConsumeOpts {
            dry_run: false,
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(report.consumed_count, 0);
    assert_eq!(report.skipped_no_draft, 1);

    let again = consume_stripe_webhook_queue(
        &company,
        ConsumeOpts {
            dry_run: false,
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(again.skipped_already, 1);
    assert_eq!(again.skipped_no_draft, 0);
}
