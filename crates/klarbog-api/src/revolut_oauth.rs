//! Revolut OAuth HTTP scaffold (ADR-008 — no full UI).

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_bank::{
    oauth_exchange_code, oauth_start, RevolutOAuthConfig, RevolutOAuthError,
};
use klarbog_types::{Actor, Envelope};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct OAuthStartQuery {
    pub company: String,
}

#[derive(Deserialize)]
pub struct OAuthCallbackBody {
    pub company: String,
    pub code: String,
}

async fn authorize_company(
    allowlist_root: &Path,
    company: &Path,
    actor: &Actor,
) -> Result<PathBuf, CoreError> {
    let path = assert_company_path(allowlist_root, company)?;
    open_existing(&path).await?.authorize(actor)?;
    Ok(path)
}

fn map_core(err: CoreError) -> (StatusCode, Envelope<Value>) {
    match err {
        CoreError::ActorDenied(tag) => (
            StatusCode::FORBIDDEN,
            Envelope::err([format!("actor not in policy: {tag}")]),
        ),
        CoreError::Path(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::Store(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        CoreError::Other(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([other.to_string()]),
        ),
    }
}

fn map_oauth(err: RevolutOAuthError) -> (StatusCode, Envelope<Value>) {
    let status = match &err {
        RevolutOAuthError::Config(_) => StatusCode::SERVICE_UNAVAILABLE,
        RevolutOAuthError::MissingCode => StatusCode::BAD_REQUEST,
        RevolutOAuthError::Exchange(_) | RevolutOAuthError::Api(_) => StatusCode::BAD_GATEWAY,
        RevolutOAuthError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Envelope::err([err.to_string()]))
}

pub async fn oauth_start_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OAuthStartQuery>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(&query.company);
    let _path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let cfg = RevolutOAuthConfig::from_env().map_err(|e| {
        let (s, env) = map_oauth(RevolutOAuthError::Config(e));
        (s, Json(env))
    })?;
    let start = oauth_start(&cfg);
    Ok(Json(Envelope::ok(serde_json::json!({
        "auth_url": start.auth_url,
        "state": start.state,
    }))))
}

pub async fn oauth_callback_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OAuthCallbackBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(&body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let cfg = RevolutOAuthConfig::from_env().map_err(|e| {
        let (s, env) = map_oauth(RevolutOAuthError::Config(e));
        (s, Json(env))
    })?;
    oauth_exchange_code(&cfg, &path, &body.code)
        .await
        .map_err(|e| {
            let (s, env) = map_oauth(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "stored": true,
        "path": "secrets/revolut.json",
    }))))
}

#[cfg(test)]
mod http_tests {
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_types::{Actor, ActorKind, Envelope};
    use serde_json::Value;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::const_new(());

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

    #[tokio::test]
    async fn oauth_start_returns_auth_url() {
        let _lock = ENV_TEST_LOCK.lock().await;
        let _env = set_oauth_env();
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
        let _lock = ENV_TEST_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let _guard = EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_ID");
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
        };
        let app = router(state);
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

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            // SAFETY: tests serialize env mutation via ENV_TEST_LOCK.
            unsafe { std::env::remove_var(key) };
            Self { key, prev }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let prev = std::env::var(key).ok();
            // SAFETY: tests serialize env mutation via ENV_TEST_LOCK.
            unsafe { std::env::set_var(key, value) };
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                // SAFETY: paired with set/unset under ENV_TEST_LOCK.
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }
}
