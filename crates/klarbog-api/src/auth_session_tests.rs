//! HTTP + unit tests for ADR-017 session cookie.

use crate::auth_session::{sign_session, verify_session};
use crate::{router, AppState};
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use klarbog_core::{default_registry, ConfirmStore};
use std::sync::Arc;
use tower::ServiceExt;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[test]
fn sign_verify_roundtrip() {
    let secret = "session-secret";
    let exp = now_unix() + 3600;
    let cookie = sign_session(secret, exp);
    assert!(verify_session(secret, &cookie, now_unix()));
    assert!(!verify_session("wrong", &cookie, now_unix()));
    assert!(!verify_session(secret, &cookie, exp + 1));
}

#[test]
fn tampered_payload_rejected() {
    let secret = "session-secret";
    let cookie = sign_session(secret, now_unix() + 3600);
    let tampered = cookie.replacen("v1.", "v1.9", 1);
    assert!(!verify_session(secret, &tampered, now_unix()));
}

fn state_session(api: &str, session: &str, secure: bool) -> AppState {
    AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: std::env::temp_dir(),
        registry: Arc::new(default_registry()),
        api_token: Some(Arc::<str>::from(api)),
        session_secret: Some(Arc::<str>::from(session)),
        session_cookie_secure: secure,
    }
}

fn state_bearer_only(api: &str) -> AppState {
    AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: std::env::temp_dir(),
        registry: Arc::new(default_registry()),
        api_token: Some(Arc::<str>::from(api)),
        session_secret: None,
        session_cookie_secure: false,
    }
}

#[tokio::test]
async fn login_sets_httponly_cookie_and_status_ok_with_cookie() {
    let app = router(state_session("api-secret", "sess-secret", false));
    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"token":"api-secret"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let set_cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("Set-Cookie");
    assert!(set_cookie.starts_with("klarbog_session="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));
    assert!(set_cookie.contains("Path=/"));
    assert!(!set_cookie.contains("Secure"));

    let cookie_pair = set_cookie.split(';').next().unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .header(header::COOKIE, cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn login_wrong_token_401() {
    let app = router(state_session("api-secret", "sess-secret", false));
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"token":"wrong"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn session_routes_absent_without_session_secret() {
    let app = router(state_bearer_only("api-secret"));
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"token":"api-secret"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn logout_clears_cookie() {
    let app = router(state_session("api-secret", "sess-secret", true));
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("Set-Cookie");
    assert!(set_cookie.contains("Max-Age=0"));
    assert!(set_cookie.contains("Secure"));
}

#[tokio::test]
async fn secure_flag_on_login_when_configured() {
    let app = router(state_session("api-secret", "sess-secret", true));
    let login = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"token":"api-secret"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let set_cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(set_cookie.contains("Secure"));
}
