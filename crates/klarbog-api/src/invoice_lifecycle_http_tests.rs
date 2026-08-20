//! HTTP tests for invoice lifecycle (status / mark-paid / mark-part-paid).

use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::{Actor, ActorKind, Envelope};
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

fn actor_headers(actor: &Actor) -> (&'static str, String) {
    let kind = match actor.kind {
        ActorKind::User => "user",
        ActorKind::Agent => "agent",
        ActorKind::System => "system",
    };
    (kind, actor.id.clone())
}

async fn seed_invoice(company_path: &std::path::Path) -> (Actor, String) {
    let owner = Actor::user("owner");
    init_company(company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(company_path, None, "Buyer ApS".into()).unwrap();
    let invoice = create_draft_from_new(
        company_path,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Consulting".into(),
            amount_minor: 5000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    (owner, invoice.id.to_string())
}

#[tokio::test]
async fn patch_status_and_mark_paid() {
    let dir = tempdir().unwrap();
    let company_path = dir.path().join("co");
    std::fs::create_dir_all(&company_path).unwrap();
    let (owner, invoice_id) = seed_invoice(&company_path).await;
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let patch_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "status": "sent",
    });
    let patch_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/invoices/status")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(patch_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_res.status(), StatusCode::OK);
    let paid_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
    });
    let paid_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/invoices/mark-paid")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(paid_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(paid_res.status(), StatusCode::OK);
    let paid_bytes = axum::body::to_bytes(paid_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let paid_env: Envelope<Value> = serde_json::from_slice(&paid_bytes).unwrap();
    let data = paid_env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "paid");
    assert!(data["journal_entry"]["legs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|l| l["party_id"].is_string()));
    assert!(data.get("confirm_token").is_none());
}

#[tokio::test]
async fn mark_paid_preview_issues_confirm_token() {
    let dir = tempdir().unwrap();
    let company_path = dir.path().join("co");
    std::fs::create_dir_all(&company_path).unwrap();
    let (owner, invoice_id) = seed_invoice(&company_path).await;
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let patch_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "status": "sent",
    });
    let patch_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/invoices/status")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(patch_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_res.status(), StatusCode::OK);
    let paid_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "preview": true,
    });
    let paid_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/invoices/mark-paid")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(paid_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(paid_res.status(), StatusCode::OK);
    let paid_bytes = axum::body::to_bytes(paid_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let paid_env: Envelope<Value> = serde_json::from_slice(&paid_bytes).unwrap();
    let data = paid_env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "paid");
    let token = data["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(data["payload_digest"].as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn mark_part_paid_preview_issues_confirm_token() {
    let dir = tempdir().unwrap();
    let company_path = dir.path().join("co");
    std::fs::create_dir_all(&company_path).unwrap();
    let (owner, invoice_id) = seed_invoice(&company_path).await;
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let patch_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "status": "sent",
    });
    let patch_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/invoices/status")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(patch_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_res.status(), StatusCode::OK);
    let part_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "amount_minor": 2000,
        "preview": true,
    });
    let part_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/invoices/mark-part-paid")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(part_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(part_res.status(), StatusCode::OK);
    let part_bytes = axum::body::to_bytes(part_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let part_env: Envelope<Value> = serde_json::from_slice(&part_bytes).unwrap();
    let data = part_env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "part_paid");
    assert_eq!(
        data["invoice"]["payments"][0]["amount_minor"].as_i64(),
        Some(2000)
    );
    let token = data["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(data["payload_digest"].as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn mark_part_paid_then_mark_paid() {
    let dir = tempdir().unwrap();
    let company_path = dir.path().join("co");
    std::fs::create_dir_all(&company_path).unwrap();
    let (owner, invoice_id) = seed_invoice(&company_path).await;
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let patch_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "status": "sent",
    });
    let patch_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/invoices/status")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(patch_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_res.status(), StatusCode::OK);

    let part_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "amount_minor": 2000,
    });
    let part_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/invoices/mark-part-paid")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(part_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(part_res.status(), StatusCode::OK);
    let part_bytes = axum::body::to_bytes(part_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let part_env: Envelope<Value> = serde_json::from_slice(&part_bytes).unwrap();
    let part_data = part_env.data.unwrap();
    assert_eq!(part_data["invoice"]["status"], "part_paid");
    assert_eq!(
        part_data["invoice"]["payments"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        part_data["invoice"]["payments"][0]["amount_minor"].as_i64(),
        Some(2000)
    );
    let leg0_amount = &part_data["journal_entry"]["legs"][0]["amount"];
    let units = leg0_amount
        .as_i64()
        .or_else(|| leg0_amount.get("units").and_then(|u| u.as_i64()));
    assert_eq!(units, Some(2000));
    assert!(part_data["journal_entry"]["legs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|l| l["party_id"].is_string()));
    assert!(part_data.get("confirm_token").is_none());

    let paid_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
    });
    let paid_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/invoices/mark-paid")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(paid_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(paid_res.status(), StatusCode::OK);
    let paid_bytes = axum::body::to_bytes(paid_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let paid_env: Envelope<Value> = serde_json::from_slice(&paid_bytes).unwrap();
    let data = paid_env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "paid");
    assert_eq!(data["invoice"]["payments"].as_array().unwrap().len(), 2);
    // Payment ledger: mark-paid suggests remaining (3000), not full total.
    let full_amount = &data["journal_entry"]["legs"][0]["amount"];
    let full_units = full_amount
        .as_i64()
        .or_else(|| full_amount.get("units").and_then(|u| u.as_i64()));
    assert_eq!(full_units, Some(3000));
    assert!(data.get("confirm_token").is_none());
}
