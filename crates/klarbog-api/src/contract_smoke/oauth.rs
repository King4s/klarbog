//! Revolut oauth refresh fail-closed smoke (slice 39).

use super::{actor_headers, EnvGuard, ENV_TEST_LOCK};
use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_types::Actor;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

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
