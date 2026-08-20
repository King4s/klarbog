use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_crm::upsert_party;
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

#[tokio::test]
async fn invoice_create_and_list() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(&company_path, None, "Buyer ApS".into()).unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let create_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "party_id": party.id.to_string(),
        "kind": "sale",
        "lines": [{
            "description": "Consulting",
            "amount_minor": 12500,
            "currency": "DKK",
        }],
    });
    let create_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/invoices/drafts")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(create_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_res.status(), StatusCode::OK);
    let create_bytes = axum::body::to_bytes(create_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_env: Envelope<Value> = serde_json::from_slice(&create_bytes).unwrap();
    let data = create_env.data.unwrap();
    let invoice_id = data["invoice"]["id"].as_str().unwrap().to_string();
    assert!(data["journal_entry"]["legs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|l| l["party_id"].is_string()));
    let list_uri = format!(
        "/api/v1/invoices/drafts?company={}",
        company_path.to_string_lossy(),
    );
    let list_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&list_uri)
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_res.status(), StatusCode::OK);
    let get_uri = format!(
        "/api/v1/invoices/drafts?company={}&invoice_id={}",
        company_path.to_string_lossy(),
        invoice_id,
    );
    let get_res = app
        .oneshot(
            Request::builder()
                .uri(&get_uri)
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn invoice_actor_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    upsert_party(&company_path, None, "Buyer".into()).unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let intruder = Actor::user("intruder");
    let (kind, id) = actor_headers(&intruder);
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "party_id": "party_buyer",
        "kind": "sale",
        "lines": [{
            "description": "Blocked",
            "amount_minor": 100,
            "currency": "DKK",
        }],
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/invoices/drafts")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
