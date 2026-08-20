//! HTTP tests: bank import preview fail-closed when API env secrets are missing.

use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_types::{Actor, ActorKind};
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
async fn bank_preview_revolut_api_missing_env() {
    let _lock = crate::ENV_TEST_LOCK.lock().await;
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
    let _guards = [
        EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN"),
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_ID"),
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_SECRET"),
    ];
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "provider": "revolut",
        "source": "api",
        "currency": "DKK",
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bank/import/preview")
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

#[tokio::test]
async fn bank_preview_stripe_api_missing_env() {
    let _lock = crate::ENV_TEST_LOCK.lock().await;
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
    let _guard = EnvGuard::unset("KLARBOG_STRIPE_SECRET_KEY");
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "provider": "stripe",
        "source": "api",
        "currency": "DKK",
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bank/import/preview")
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
    assert!(!body_str.contains("\"secret_key\""));
    assert!(!body_str.contains("sk_"));
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
