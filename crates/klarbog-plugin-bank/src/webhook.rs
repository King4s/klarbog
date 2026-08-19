//! Stripe webhook verify + queue (ADR-009 phase). Draft rows only — no journal post.

use crate::config::{BankApiConfigError, StripeWebhookConfig};
// StripeWebhookConfig lives in config.rs (KLARBOG_STRIPE_WEBHOOK_SECRET).
use crate::csv::BankRow;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use klarbog_types::MinorAmount;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::path::Path;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

pub const STRIPE_WEBHOOKS_DIR: &str = "stripe_webhooks";
pub const STRIPE_WEBHOOKS_QUEUE: &str = "stripe_webhooks/queue.jsonl";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StripeWebhookError {
    #[error("missing Stripe-Signature header")]
    MissingSignature,
    #[error("invalid Stripe-Signature header")]
    InvalidSignatureHeader,
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("timestamp outside tolerance")]
    TimestampSkew,
    #[error("invalid webhook json: {0}")]
    InvalidJson(String),
    #[error("unsupported event type: {0}")]
    UnsupportedEvent(String),
    #[error("event object missing field: {0}")]
    MissingField(&'static str),
    #[error(transparent)]
    Config(#[from] BankApiConfigError),
    #[error("io error: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StripeWebhookDraft {
    pub date: DateTime<Utc>,
    pub text: String,
    pub amount_minor: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuedStripeWebhook {
    pub event_id: String,
    pub event_type: String,
    pub object_id: String,
    pub received_at: DateTime<Utc>,
    pub draft: Option<StripeWebhookDraft>,
}

pub fn verify_stripe_signature(
    payload: &[u8],
    sig_header: &str,
    secret: &str,
    tolerance_secs: i64,
) -> Result<(), StripeWebhookError> {
    let (timestamp, signatures) = parse_signature_header(sig_header)?;
    let now = Utc::now().timestamp();
    if (now - timestamp).abs() > tolerance_secs {
        return Err(StripeWebhookError::TimestampSkew);
    }
    let signed = format!(
        "{timestamp}.{}",
        std::str::from_utf8(payload).map_err(|e| StripeWebhookError::InvalidJson(e.to_string()))?
    );
    let expected = compute_signature(secret, &signed);
    if signatures
        .iter()
        .any(|sig| constant_time_eq(sig, &expected))
    {
        Ok(())
    } else {
        Err(StripeWebhookError::SignatureMismatch)
    }
}

pub fn sign_test_payload(payload: &[u8], secret: &str, timestamp: i64) -> String {
    let body = std::str::from_utf8(payload).expect("utf8 fixture");
    let signed = format!("{timestamp}.{body}");
    let sig = compute_signature(secret, &signed);
    format!("t={timestamp},v1={sig}")
}

fn compute_signature(secret: &str, signed_payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(signed_payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn parse_signature_header(header: &str) -> Result<(i64, Vec<String>), StripeWebhookError> {
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in header.split(',') {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let val = kv.next().unwrap_or("").trim();
        match key {
            "t" => {
                timestamp = Some(
                    val.parse::<i64>()
                        .map_err(|_| StripeWebhookError::InvalidSignatureHeader)?,
                );
            }
            "v1" => signatures.push(val.to_string()),
            _ => {}
        }
    }
    let Some(ts) = timestamp else {
        return Err(StripeWebhookError::InvalidSignatureHeader);
    };
    if signatures.is_empty() {
        return Err(StripeWebhookError::InvalidSignatureHeader);
    }
    Ok((ts, signatures))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub fn parse_webhook_event(json: &str) -> Result<(String, String, Value), StripeWebhookError> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| StripeWebhookError::InvalidJson(e.to_string()))?;
    let event_id = root
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or(StripeWebhookError::MissingField("id"))?
        .to_string();
    let event_type = root
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or(StripeWebhookError::MissingField("type"))?
        .to_string();
    Ok((event_id, event_type, root))
}

pub fn draft_from_event(
    event_type: &str,
    root: &Value,
) -> Result<StripeWebhookDraft, StripeWebhookError> {
    let obj = root
        .pointer("/data/object")
        .ok_or(StripeWebhookError::MissingField("data.object"))?;
    match event_type {
        "payout.paid" => draft_from_payout(obj),
        "charge.succeeded" => draft_from_charge(obj),
        other => Err(StripeWebhookError::UnsupportedEvent(other.to_string())),
    }
}

fn draft_from_payout(obj: &Value) -> Result<StripeWebhookDraft, StripeWebhookError> {
    let amount = obj
        .get("amount")
        .and_then(|v| v.as_i64())
        .ok_or(StripeWebhookError::MissingField("amount"))?;
    let currency = currency_code(obj)?;
    let ts = obj
        .get("arrival_date")
        .or_else(|| obj.get("created"))
        .and_then(|v| v.as_i64())
        .ok_or(StripeWebhookError::MissingField("arrival_date"))?;
    let date = timestamp_to_utc(ts)?;
    let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("payout");
    Ok(StripeWebhookDraft {
        date,
        text: format!("Stripe payout {id}"),
        amount_minor: -amount.abs(),
        currency,
    })
}

fn draft_from_charge(obj: &Value) -> Result<StripeWebhookDraft, StripeWebhookError> {
    let amount = obj
        .get("amount")
        .and_then(|v| v.as_i64())
        .ok_or(StripeWebhookError::MissingField("amount"))?;
    let currency = currency_code(obj)?;
    let ts = obj
        .get("created")
        .and_then(|v| v.as_i64())
        .ok_or(StripeWebhookError::MissingField("created"))?;
    let date = timestamp_to_utc(ts)?;
    let text = obj
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| format!("Stripe charge: {s}"))
        .or_else(|| {
            obj.get("id")
                .and_then(|v| v.as_str())
                .map(|id| format!("Stripe charge {id}"))
        })
        .unwrap_or_else(|| "Stripe charge".into());
    Ok(StripeWebhookDraft {
        date,
        text,
        amount_minor: amount.abs(),
        currency,
    })
}

fn currency_code(obj: &Value) -> Result<String, StripeWebhookError> {
    obj.get("currency")
        .and_then(|v| v.as_str())
        .map(|s| s.to_uppercase())
        .ok_or(StripeWebhookError::MissingField("currency"))
}

fn timestamp_to_utc(ts: i64) -> Result<DateTime<Utc>, StripeWebhookError> {
    DateTime::from_timestamp(ts, 0).ok_or(StripeWebhookError::MissingField("timestamp"))
}

pub fn draft_to_bank_row(draft: &StripeWebhookDraft) -> BankRow {
    BankRow {
        date: draft.date,
        text: draft.text.clone(),
        amount_minor: MinorAmount::from_minor(draft.amount_minor),
    }
}

pub fn queue_stripe_webhook(
    company: &Path,
    entry: &QueuedStripeWebhook,
) -> Result<(), StripeWebhookError> {
    let dir = company.join(STRIPE_WEBHOOKS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    let line =
        serde_json::to_string(entry).map_err(|e| StripeWebhookError::InvalidJson(e.to_string()))?;
    use std::io::Write;
    let path = company.join(STRIPE_WEBHOOKS_QUEUE);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    writeln!(file, "{line}").map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    Ok(())
}

pub fn ingest_stripe_webhook(
    company: &Path,
    payload: &[u8],
    sig_header: Option<&str>,
    cfg: &StripeWebhookConfig,
) -> Result<QueuedStripeWebhook, StripeWebhookError> {
    let sig = sig_header.ok_or(StripeWebhookError::MissingSignature)?;
    verify_stripe_signature(payload, sig, &cfg.webhook_secret, 300)?;
    let json =
        std::str::from_utf8(payload).map_err(|e| StripeWebhookError::InvalidJson(e.to_string()))?;
    let (event_id, event_type, root) = parse_webhook_event(json)?;
    let object_id = root
        .pointer("/data/object/id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let draft = match draft_from_event(&event_type, &root) {
        Ok(d) => Some(d),
        Err(StripeWebhookError::UnsupportedEvent(_)) => None,
        Err(e) => return Err(e),
    };
    let entry = QueuedStripeWebhook {
        event_id,
        event_type,
        object_id,
        received_at: Utc::now(),
        draft,
    };
    queue_stripe_webhook(company, &entry)?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const PAYOUT_FIXTURE: &str = include_str!("../tests/fixtures/stripe_webhook_payout_paid.json");
    const CHARGE_FIXTURE: &str =
        include_str!("../tests/fixtures/stripe_webhook_charge_succeeded.json");
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
        let err =
            verify_stripe_signature(PAYOUT_FIXTURE.as_bytes(), &sig, "wrong", 300).unwrap_err();
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
}
