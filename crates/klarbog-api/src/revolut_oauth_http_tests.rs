//! HTTP tests for Revolut OAuth routes (fail-closed; never assert raw tokens in JSON).

use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_bank::{save_revolut_tokens, RevolutStoredTokens};
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

fn set_oauth_env() -> Vec<EnvGuard> {
    vec![
        EnvGuard::set("KLARBOG_REVOLUT_CLIENT_ID", "client-id"),
        EnvGuard::set("KLARBOG_REVOLUT_CLIENT_SECRET", "client-secret"),
        EnvGuard::set(
            "KLARBOG_REVOLUT_REDIRECT_URI",
            "https://example.test/callback",
        ),
        EnvGuard::set("KLARBOG_REVOLUT_TOKEN_URL", "https://example.test/token"),
    ]
}

async fn app_with_company() -> (axum::Router, std::path::PathBuf, Actor, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
    };
    (router(state), company_path, owner, dir)
}

#[tokio::test]
async fn oauth_start_returns_auth_url() {
    let _lock = crate::ENV_TEST_LOCK.lock().await;
    let _env = set_oauth_env();
    let (app, company_path, owner, _dir) = app_with_company().await;
    let (kind, id) = actor_headers(&owner);
    let uri = format!(
        "/api/v1/revolut/oauth/start?company={}",
        company_path.to_string_lossy()
    );
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::empty())
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
    assert!(data["auth_url"]
        .as_str()
        .unwrap()
        .contains("client_id=client-id"));
    assert!(data["state"].as_str().unwrap().len() >= 8);
    let body_str = std::str::from_utf8(bytes.as_ref()).unwrap();
    assert!(!body_str.contains("client-secret"));
}

#[tokio::test]
async fn oauth_start_missing_config_is_fail_closed() {
    let _lock = crate::ENV_TEST_LOCK.lock().await;
    let (app, company_path, owner, _dir) = app_with_company().await;
    let _guard = EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_ID");
    let (kind, id) = actor_headers(&owner);
    let uri = format!(
        "/api/v1/revolut/oauth/start?company={}",
        company_path.to_string_lossy()
    );
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn oauth_refresh_fail_closed_without_refresh_token() {
    let _lock = crate::ENV_TEST_LOCK.lock().await;
    let _env = set_oauth_env();
    let (app, company_path, owner, _dir) = app_with_company().await;
    save_revolut_tokens(
        &company_path,
        &RevolutStoredTokens {
            access_token: "access-only".into(),
            refresh_token: None,
            expires_at: Some(1),
            token_type: None,
        },
    )
    .unwrap();
    let (kind, id) = actor_headers(&owner);
    let body = serde_json::json!({ "company": company_path.to_string_lossy() });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/revolut/oauth/refresh")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = std::str::from_utf8(bytes.as_ref()).unwrap();
    assert!(!body_str.contains("access-only"));
    assert!(!body_str.contains("\"access_token\""));
    assert!(!body_str.contains("\"refresh_token\""));
}

#[tokio::test]
async fn oauth_refresh_unauthorized_actor_forbidden() {
    let _lock = crate::ENV_TEST_LOCK.lock().await;
    let _env = set_oauth_env();
    let (app, company_path, _owner, _dir) = app_with_company().await;
    let stranger = Actor::user("stranger");
    let (kind, id) = actor_headers(&stranger);
    let body = serde_json::json!({ "company": company_path.to_string_lossy() });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/revolut/oauth/refresh")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
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

    fn set(key: &'static str, value: &str) -> Self {
        let prev = std::env::var(key).ok();
        unsafe { std::env::set_var(key, value) };
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
