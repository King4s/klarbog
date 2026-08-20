//! Optional API bearer gate (ADR-016) + session cookie (ADR-017).

use crate::auth_session;
use crate::AppState;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use klarbog_types::Envelope;
use subtle::ConstantTimeEq;

const HEADER_API_TOKEN: &str = "x-klarbog-api-token";

fn path_exempt(path: &str) -> bool {
    path == "/health"
        || path == "/ui"
        || path.starts_with("/ui/")
        || path == "/api/v1/webhooks/stripe"
        || path == "/api/v1/auth/login"
        || path == "/api/v1/auth/logout"
}

fn extract_presented(req: &Request<axum::body::Body>) -> Option<String> {
    if let Some(raw) = req
        .headers()
        .get(HEADER_API_TOKEN)
        .and_then(|v| v.to_str().ok())
    {
        let t = raw.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())?;
    let auth = auth.trim();
    let bearer = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))?;
    let t = bearer.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn tokens_equal(presented: &str, expected: &str) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    presented.as_bytes().ct_eq(expected.as_bytes()).into()
}

pub async fn api_token_middleware(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
    if path_exempt(path) {
        return next.run(req).await;
    }
    let Some(expected) = state.api_token.as_deref() else {
        return next.run(req).await;
    };
    if expected.is_empty() {
        return next.run(req).await;
    }
    if let Some(presented) = extract_presented(&req) {
        if tokens_equal(&presented, expected) {
            return next.run(req).await;
        }
    }
    if let Some(secret) = state.session_secret.as_deref().filter(|s| !s.is_empty()) {
        if auth_session::session_cookie_valid(req.headers(), secret) {
            return next.run(req).await;
        }
    }
    (
        StatusCode::UNAUTHORIZED,
        Json(Envelope::<serde_json::Value>::err([
            "missing or invalid API token (Authorization: Bearer …, x-klarbog-api-token, or klarbog_session cookie)",
        ])),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{path_exempt, tokens_equal};
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, ConfirmStore};
    use std::sync::Arc;
    use tower::ServiceExt;

    #[test]
    fn exempt_paths() {
        assert!(path_exempt("/health"));
        assert!(path_exempt("/ui"));
        assert!(path_exempt("/ui/"));
        assert!(path_exempt("/ui/app.js"));
        assert!(path_exempt("/api/v1/webhooks/stripe"));
        assert!(path_exempt("/api/v1/auth/login"));
        assert!(path_exempt("/api/v1/auth/logout"));
        assert!(!path_exempt("/api/v1/status"));
        assert!(!path_exempt("/api/v1/crm/parties"));
    }

    #[test]
    fn constant_time_compare() {
        assert!(tokens_equal("secret-token", "secret-token"));
        assert!(!tokens_equal("secret-token", "secret-tokem"));
        assert!(!tokens_equal("short", "longer-value"));
    }

    fn state_with_token(token: Option<&str>) -> AppState {
        AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: std::env::temp_dir(),
            registry: Arc::new(default_registry()),
            api_token: token.map(Arc::<str>::from),
            session_secret: None,
            session_cookie_secure: false,
        }
    }

    #[tokio::test]
    async fn status_ok_without_token_when_unset() {
        let app = router(state_with_token(None));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn status_401_when_token_required_and_missing() {
        let app = router(state_with_token(Some("test-secret")));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn status_ok_with_bearer_when_token_required() {
        let app = router(state_with_token(Some("test-secret")));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/status")
                    .header("Authorization", "Bearer test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_exempt_when_token_required() {
        let app = router(state_with_token(Some("test-secret")));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
