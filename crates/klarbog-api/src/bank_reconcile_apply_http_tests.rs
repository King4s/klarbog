//! HTTP tests for POST /api/v1/bank/reconcile/apply (entry + ConfirmStore preview).

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

async fn seeded_apply_app() -> (
    axum::Router,
    tempfile::TempDir,
    std::path::PathBuf,
    String,
    Actor,
) {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &company_path,
        None,
        "Nordic Supply".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &company_path,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: 50_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    (router(state), dir, company_path, inv.id.to_string(), owner)
}

#[tokio::test]
async fn reconcile_apply_returns_entry_json() {
    let (app, _dir, company_path, invoice_id, owner) = seeded_apply_app().await;
    let (kind, id) = actor_headers(&owner);
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "row": {
            "date": "2026-05-20",
            "text": "Customer payment Nordic Supply consulting",
            "amount_minor": 50000
        }
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bank/reconcile/apply")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    let data = env.data.unwrap();
    assert!(data["entry"]["memo"].as_str().unwrap().contains("bank:"));
    assert!(data["entry"]["legs"].as_array().unwrap()[0]["party_id"].is_string());
    assert_eq!(data["forced"], false);
    assert!(data.get("confirm_token").is_none());
}

#[tokio::test]
async fn reconcile_apply_preview_issues_confirm_token() {
    let (app, _dir, company_path, invoice_id, owner) = seeded_apply_app().await;
    let (kind, id) = actor_headers(&owner);
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": invoice_id,
        "preview": true,
        "row": {
            "date": "2026-05-20",
            "text": "Customer payment Nordic Supply consulting",
            "amount_minor": 50000
        }
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bank/reconcile/apply")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    let data = env.data.unwrap();
    assert!(data["entry"]["memo"].as_str().unwrap().contains("bank:"));
    let token = data["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["expires_unix_ms"].as_u64().unwrap() > 0);
    assert!(data["payload_digest"].as_str().unwrap().len() == 64);
}
