//! HTTP tests for Stripe consume → unique safe apply-preview (ConfirmStore).

use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_bank::{
    ingest_stripe_webhook, sign_test_payload, StripeWebhookConfig, STRIPE_WEBHOOKS_CONSUMED,
};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::{Actor, Envelope};
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

const CHARGE_FIXTURE: &str =
    include_str!("../../klarbog-plugin-bank/tests/fixtures/stripe_webhook_charge_succeeded.json");
const TEST_SECRET: &str = "whsec_test_fixture_secret";

fn actor_headers() -> [(&'static str, &'static str); 2] {
    [
        ("x-klarbog-actor-kind", "user"),
        ("x-klarbog-actor-id", "owner"),
    ]
}

async fn company_with_charge(match_amount: bool) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(&company_path, None, "Customer payment".into()).unwrap();
    let amount = if match_amount { 24_275 } else { 99_999 };
    create_draft_from_new(
        &company_path,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: amount,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let cfg = StripeWebhookConfig {
        webhook_secret: TEST_SECRET.into(),
    };
    let ts = chrono::Utc::now().timestamp();
    let sig = sign_test_payload(CHARGE_FIXTURE.as_bytes(), TEST_SECRET, ts);
    ingest_stripe_webhook(&company_path, CHARGE_FIXTURE.as_bytes(), Some(&sig), &cfg).unwrap();
    (dir, company_path)
}

#[tokio::test]
async fn unique_safe_issues_confirm_token_no_journal_post() {
    let (dir, company_path) = company_with_charge(true).await;
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let body = serde_json::json!({ "company": company_path.to_string_lossy() });
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/v1/bank/stripe/reconcile-apply-preview")
        .header("content-type", "application/json");
    for (k, v) in actor_headers() {
        req = req.header(k, v);
    }
    let res = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    let data = env.data.unwrap();
    assert_eq!(data["consume"]["dry_run"], true);
    assert!(data["applied"].is_object());
    let token = data["applied"]["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["applied"]["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(
        data["applied"]["payload_digest"].as_str().unwrap().len(),
        64
    );
    assert!(data["applied"]["entry"]["memo"]
        .as_str()
        .unwrap()
        .contains("bank:"));
    assert!(!company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
}

#[tokio::test]
async fn no_safe_match_returns_suggestions_without_apply() {
    let (dir, company_path) = company_with_charge(false).await;
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let body = serde_json::json!({ "company": company_path.to_string_lossy() });
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/v1/bank/stripe/reconcile-apply-preview")
        .header("content-type", "application/json");
    for (k, v) in actor_headers() {
        req = req.header(k, v);
    }
    let res = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    let data = env.data.unwrap();
    assert!(data["applied"].is_null());
    assert_eq!(data["matches"][0]["amount_minor"], 24_275);
    assert!(data.get("confirm_token").is_none());
}

#[tokio::test]
async fn confirm_consume_persists_sidecar() {
    let (dir, company_path) = company_with_charge(true).await;
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "confirm_consume": true
    });
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/v1/bank/stripe/reconcile-apply-preview")
        .header("content-type", "application/json");
    for (k, v) in actor_headers() {
        req = req.header(k, v);
    }
    let res = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    let data = env.data.unwrap();
    assert_eq!(data["consume"]["dry_run"], false);
    assert!(data["applied"].is_object());
    assert!(company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
}
