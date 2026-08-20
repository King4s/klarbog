use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_retention::DEFAULT_RETAIN_DAYS;
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
async fn retention_backup_gdpr_flow() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
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
    let retention_uri = format!(
        "/api/v1/retention?company={}",
        company_path.to_string_lossy(),
    );
    let retention_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&retention_uri)
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retention_res.status(), StatusCode::OK);
    let retention_bytes = axum::body::to_bytes(retention_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let retention_env: Envelope<Value> = serde_json::from_slice(&retention_bytes).unwrap();
    assert_eq!(
        retention_env.data.unwrap()["retain_days"].as_i64().unwrap(),
        DEFAULT_RETAIN_DAYS
    );
    let backup_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
    });
    let backup_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/backup")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(backup_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(backup_res.status(), StatusCode::OK);
    let backup_bytes = axum::body::to_bytes(backup_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let backup_env: Envelope<Value> = serde_json::from_slice(&backup_bytes).unwrap();
    assert!(backup_env.data.unwrap()["backup_key"].is_string());
    let gdpr_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
    });
    let gdpr_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/gdpr-export")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(gdpr_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(gdpr_res.status(), StatusCode::OK);
    let gdpr_bytes = axum::body::to_bytes(gdpr_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let gdpr_env: Envelope<Value> = serde_json::from_slice(&gdpr_bytes).unwrap();
    let gdpr = gdpr_env.data.unwrap();
    assert!(gdpr["exported_unix_ms"].is_number());
    assert!(gdpr["note"].as_str().unwrap().contains("immutable"));
    assert!(gdpr["parties"].is_array());
    assert!(gdpr["invoices"].is_array());
    assert!(gdpr["documents"].is_array());
    assert!(gdpr["exceptions"].is_array());
    assert!(gdpr["retention"]["retain_days"].is_number());
    assert!(company_path.join("gdpr_export.json").exists());
}

#[tokio::test]
async fn retention_actor_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
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
    let uri = format!(
        "/api/v1/retention?company={}",
        company_path.to_string_lossy(),
    );
    let res = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
