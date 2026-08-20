//! Full offline contract smoke flow (slice 35 + 39 remaining-minor).

use super::{actor_headers, json_req, PAYOUT_FIXTURE, TEST_SECRET};
use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_bank::{
    ingest_stripe_webhook, sign_test_payload, StripeWebhookConfig, STRIPE_WEBHOOKS_CONSUMED,
};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::Actor;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

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
    // default (no preview): suggestion only — no ConfirmStore fields
    assert!(part.get("confirm_token").is_none());
    assert!(part.get("expires_unix_ms").is_none());
    assert!(part.get("payload_digest").is_none());
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
    assert!(paid.get("confirm_token").is_none());
    assert!(paid.get("expires_unix_ms").is_none());
    assert!(paid.get("payload_digest").is_none());
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
