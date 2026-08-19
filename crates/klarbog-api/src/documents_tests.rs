use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
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
async fn documents_and_exceptions_flow() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let attach_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "kind": "receipt",
        "path_hint": "attachments/r.pdf",
        "notes": "test",
    });
    let attach_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/documents")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(attach_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(attach_res.status(), StatusCode::OK);
    let attach_bytes = axum::body::to_bytes(attach_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let attach_env: Envelope<Value> = serde_json::from_slice(&attach_bytes).unwrap();
    let doc_id = attach_env.data.unwrap()["id"].as_str().unwrap().to_string();
    let raise_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "code": "missing_attachment",
        "severity": "warn",
        "message": "No scan linked",
        "related_ids": [doc_id.clone()],
    });
    let raise_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/exceptions")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(raise_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(raise_res.status(), StatusCode::OK);
    let raise_bytes = axum::body::to_bytes(raise_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let raise_env: Envelope<Value> = serde_json::from_slice(&raise_bytes).unwrap();
    let exc_id = raise_env.data.unwrap()["id"].as_str().unwrap().to_string();
    let list_uri = format!(
        "/api/v1/exceptions?company={}",
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
    let close_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "exception_id": exc_id,
        "open": false,
    });
    let close_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/exceptions")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(close_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(close_res.status(), StatusCode::OK);
    let bad_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "kind": "other",
        "path_hint": "../bad.pdf",
    });
    let bad_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/documents")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(bad_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_document_removes_metadata_and_object() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let payload = b"to-delete";
    let attach_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "kind": "receipt",
        "path_hint": "attachments/del.bin",
        "content_base64": base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            payload,
        ),
    });
    let attach_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/documents")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(attach_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(attach_res.status(), StatusCode::OK);
    let attach_bytes = axum::body::to_bytes(attach_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let attach_env: Envelope<Value> = serde_json::from_slice(&attach_bytes).unwrap();
    let doc_id = attach_env.data.unwrap()["id"].as_str().unwrap().to_string();
    let stored = company_path.join("objects/attachments/del.bin");
    assert_eq!(std::fs::read(&stored).unwrap(), payload);
    let delete_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "document_id": doc_id,
    });
    let delete_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/documents")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(delete_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_res.status(), StatusCode::OK);
    assert!(!stored.exists());
    let list_uri = format!(
        "/api/v1/documents?company={}",
        company_path.to_string_lossy(),
    );
    let list_res = app
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
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_env: Envelope<Value> = serde_json::from_slice(&list_bytes).unwrap();
    assert!(list_env.data.unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn attach_with_content_base64_puts_object() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let payload = b"receipt-bytes";
    let attach_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "kind": "receipt",
        "path_hint": "attachments/receipt.bin",
        "content_base64": base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            payload,
        ),
    });
    let attach_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/documents")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(attach_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(attach_res.status(), StatusCode::OK);
    let stored = company_path.join("objects").join("attachments/receipt.bin");
    assert_eq!(std::fs::read(stored).unwrap(), payload);
}
