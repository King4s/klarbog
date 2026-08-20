//! Wave35: invoice mark-paid / mark-part-paid ConfirmStore preview in contract smoke.

use super::{actor_headers, json_req};
use crate::{router, AppState};
use axum::http::StatusCode;
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_types::Actor;
use std::sync::Arc;
use tempfile::tempdir;

fn assert_confirm_store(data: &serde_json::Value) {
    let token = data["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(data["payload_digest"].as_str().unwrap().len(), 64);
}

/// Offline smoke: `preview: true` issues ConfirmStore fields; i64 amount_minor.
#[tokio::test]
async fn contract_smoke_invoice_mark_paid_preview() {
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

    let (st, party) = json_req(
        app.clone(),
        "POST",
        "/api/v1/crm/parties",
        kind,
        &id,
        serde_json::json!({ "company": company, "display_name": "Preview Co" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let party_id = party["id"].as_str().unwrap();

    // mark-part-paid with preview:true
    let (st, draft_part) = json_req(
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
                "description": "Part preview",
                "amount_minor": 10_000_i64,
                "currency": "DKK"
            }]
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let part_id = draft_part["invoice"]["id"].as_str().unwrap();
    let (st, _) = json_req(
        app.clone(),
        "PATCH",
        "/api/v1/invoices/status",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "invoice_id": part_id,
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
            "invoice_id": part_id,
            "amount_minor": 2_500_i64,
            "preview": true
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(part["invoice"]["status"], "part_paid");
    assert_eq!(
        part["invoice"]["payments"][0]["amount_minor"].as_i64(),
        Some(2_500)
    );
    assert_confirm_store(&part);

    // mark-paid with preview:true (separate invoice)
    let (st, draft_paid) = json_req(
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
                "description": "Paid preview",
                "amount_minor": 8_000_i64,
                "currency": "DKK"
            }]
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let paid_id = draft_paid["invoice"]["id"].as_str().unwrap();
    let (st, _) = json_req(
        app.clone(),
        "PATCH",
        "/api/v1/invoices/status",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "invoice_id": paid_id,
            "status": "sent"
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, paid) = json_req(
        app,
        "POST",
        "/api/v1/invoices/mark-paid",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "invoice_id": paid_id,
            "preview": true
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(paid["invoice"]["status"], "paid");
    assert_confirm_store(&paid);
}
