//! Offline HTTP contract smoke (slice 35 + 39).
//!
//! Uses axum `oneshot` against a temp company — no TCP bind, no network.

use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_bank::{
    ingest_stripe_webhook, sign_test_payload, StripeWebhookConfig, STRIPE_WEBHOOKS_CONSUMED,
};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::{Actor, ActorKind, Envelope};
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::Mutex;
use tower::ServiceExt;

/// Serializes env mutation across oauth fail-closed smoke (slice 39).
static ENV_TEST_LOCK: Mutex<()> = Mutex::const_new(());

const PAYOUT_FIXTURE: &str =
    include_str!("../../klarbog-plugin-bank/tests/fixtures/stripe_webhook_payout_paid.json");
const TEST_SECRET: &str = "whsec_test_fixture_secret";

fn actor_headers(actor: &Actor) -> (&'static str, String) {
    let kind = match actor.kind {
        ActorKind::User => "user",
        ActorKind::Agent => "agent",
        ActorKind::System => "system",
    };
    (kind, actor.id.clone())
}

async fn json_req(
    app: axum::Router,
    method: &str,
    uri: &str,
    kind: &str,
    id: &str,
    body: Value,
) -> (StatusCode, Value) {
    let res = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    (status, env.data.unwrap_or(Value::Null))
}

/// Revolut oauth refresh fail-closed (503) when client env tokens are absent.
#[tokio::test]
async fn contract_smoke_oauth_refresh_fail_closed() {
    let _lock = ENV_TEST_LOCK.lock().await;
    let _guards = [
        EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN"),
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_ID"),
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_SECRET"),
    ];
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let (kind, id) = actor_headers(&owner);
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let body = serde_json::json!({ "company": company_path.to_string_lossy() });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/revolut/oauth/refresh")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = std::str::from_utf8(bytes.as_ref()).unwrap();
    assert!(!body_str.contains("\"access_token\""));
    assert!(!body_str.contains("\"refresh_token\""));
}

/// Full offline contract smoke: health → CRM → invoice → part-paid → mark-paid
/// remaining → reconcile → GDPR export → erase dry-run → stripe consume dry-run.
#[tokio::test]
async fn contract_smoke() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let company = company_path.to_string_lossy().to_string();
    let (kind, id) = actor_headers(&owner);

    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);

    // health
    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    // crm upsert
    let (st, party) = json_req(
        app.clone(),
        "POST",
        "/api/v1/crm/parties",
        kind,
        &id,
        serde_json::json!({ "company": company, "display_name": "Smoke Co" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let party_id = party["id"].as_str().unwrap().to_string();

    // invoice draft
    let (st, draft) = json_req(
        app.clone(),
        "POST",
        "/api/v1/invoices/drafts",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "party_id": party_id,
            "kind": "sale",
            "lines": [{
                "description": "Smoke line",
                "amount_minor": 10_000,
                "currency": "DKK"
            }]
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let invoice_id = draft["invoice"]["id"].as_str().unwrap().to_string();

    // mark-part-paid (requires sent)
    let (st, _) = json_req(
        app.clone(),
        "PATCH",
        "/api/v1/invoices/status",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "invoice_id": invoice_id,
            "status": "sent"
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, part) = json_req(
        app.clone(),
        "POST",
        "/api/v1/invoices/mark-part-paid",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "invoice_id": invoice_id,
            "amount_minor": 2500
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(part["invoice"]["status"], "part_paid");
    // after part_paid (2500 of 10000), mark-paid must suggest remaining 7500
    let (st, paid) = json_req(
        app.clone(),
        "POST",
        "/api/v1/invoices/mark-paid",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "invoice_id": invoice_id
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(paid["invoice"]["status"], "paid");
    let remaining_leg = &paid["journal_entry"]["legs"][0]["amount"];
    let remaining_units = remaining_leg
        .as_i64()
        .or_else(|| remaining_leg.get("units").and_then(|u| u.as_i64()));
    assert_eq!(remaining_units, Some(7_500));
    if let Some(rm) = paid.get("remaining_minor").and_then(|v| v.as_i64()) {
        assert_eq!(rm, 0);
    }

    // reconcile suggest (open sale + matching bank row)
    let party2 = upsert_party(&company_path, None, "Nordic Supply".into()).unwrap();
    create_draft_from_new(
        &company_path,
        party2.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: 50_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let (st, recon) = json_req(
        app.clone(),
        "POST",
        "/api/v1/bank/reconcile/suggest",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "rows": [{
                "date": "2026-05-20",
                "text": "Customer payment Nordic Supply consulting",
                "amount_minor": 50000
            }]
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(recon["count"].as_u64().unwrap() >= 1);

    // gdpr export
    let (st, gdpr) = json_req(
        app.clone(),
        "POST",
        "/api/v1/gdpr-export",
        kind,
        &id,
        serde_json::json!({ "company": company }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(gdpr["parties"].is_array());
    assert!(company_path.join("gdpr_export.json").exists());

    // erase-party dry-run
    let (st, erase) = json_req(
        app.clone(),
        "POST",
        "/api/v1/gdpr/erase-party",
        kind,
        &id,
        serde_json::json!({ "company": company, "party_id": party_id }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(erase["dry_run"], true);

    // stripe consume dry-run
    let cfg = StripeWebhookConfig {
        webhook_secret: TEST_SECRET.into(),
    };
    let ts = chrono::Utc::now().timestamp();
    let sig = sign_test_payload(PAYOUT_FIXTURE.as_bytes(), TEST_SECRET, ts);
    ingest_stripe_webhook(&company_path, PAYOUT_FIXTURE.as_bytes(), Some(&sig), &cfg).unwrap();
    let (st, consume) = json_req(
        app,
        "POST",
        "/api/v1/bank/stripe/consume",
        kind,
        &id,
        serde_json::json!({ "company": company }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(consume["dry_run"], true);
    assert!(consume["consumed_count"].as_u64().unwrap() >= 1);
    assert!(!company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
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
