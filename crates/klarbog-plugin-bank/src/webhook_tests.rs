//! Unit tests for Stripe webhook verify + queue (ADR-009).

use crate::config::{BankApiConfigError, StripeWebhookConfig};
use crate::webhook::{
    draft_from_event, draft_to_bank_row, ingest_stripe_webhook, parse_webhook_event,
    sign_test_payload, verify_stripe_signature, StripeWebhookError, STRIPE_WEBHOOKS_QUEUE,
};
use chrono::Utc;
use tempfile::tempdir;

const PAYOUT_FIXTURE: &str = include_str!("../tests/fixtures/stripe_webhook_payout_paid.json");
const CHARGE_FIXTURE: &str = include_str!("../tests/fixtures/stripe_webhook_charge_succeeded.json");
const TEST_SECRET: &str = "whsec_test_fixture_secret";

#[test]
fn verify_fixture_hmac() {
    let ts = Utc::now().timestamp();
    let sig = sign_test_payload(PAYOUT_FIXTURE.as_bytes(), TEST_SECRET, ts);
    verify_stripe_signature(PAYOUT_FIXTURE.as_bytes(), &sig, TEST_SECRET, 300).unwrap();
}

#[test]
fn rejects_bad_signature() {
    let ts = Utc::now().timestamp();
    let sig = sign_test_payload(PAYOUT_FIXTURE.as_bytes(), TEST_SECRET, ts);
    let err = verify_stripe_signature(PAYOUT_FIXTURE.as_bytes(), &sig, "wrong", 300).unwrap_err();
    assert_eq!(err, StripeWebhookError::SignatureMismatch);
}

#[test]
fn payout_paid_draft_is_negative() {
    let (_, event_type, root) = parse_webhook_event(PAYOUT_FIXTURE).unwrap();
    let draft = draft_from_event(&event_type, &root).unwrap();
    assert_eq!(draft.amount_minor, -100_000);
    assert_eq!(draft.currency, "DKK");
    assert!(draft.text.contains("payout"));
}

#[test]
fn charge_succeeded_draft_is_positive() {
    let (_, event_type, root) = parse_webhook_event(CHARGE_FIXTURE).unwrap();
    let draft = draft_from_event(&event_type, &root).unwrap();
    assert_eq!(draft.amount_minor, 24_275);
    assert_eq!(draft.currency, "DKK");
}

#[test]
fn queues_under_company_dir() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    let ts = Utc::now().timestamp();
    let sig = sign_test_payload(PAYOUT_FIXTURE.as_bytes(), TEST_SECRET, ts);
    let cfg = StripeWebhookConfig {
        webhook_secret: TEST_SECRET.into(),
    };
    let entry =
        ingest_stripe_webhook(&company, PAYOUT_FIXTURE.as_bytes(), Some(&sig), &cfg).unwrap();
    assert_eq!(entry.event_type, "payout.paid");
    let queue_path = company.join(STRIPE_WEBHOOKS_QUEUE);
    assert!(queue_path.exists());
    let text = std::fs::read_to_string(queue_path).unwrap();
    assert!(text.contains("payout.paid"));
    let row = draft_to_bank_row(entry.draft.as_ref().unwrap());
    assert_eq!(row.amount_minor.minor(), -100_000);
}

#[test]
fn config_from_env_requires_secret() {
    let _guard = EnvGuard::unset("KLARBOG_STRIPE_WEBHOOK_SECRET");
    let err = StripeWebhookConfig::from_env().unwrap_err();
    assert_eq!(
        err,
        BankApiConfigError::MissingEnv("KLARBOG_STRIPE_WEBHOOK_SECRET")
    );
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
