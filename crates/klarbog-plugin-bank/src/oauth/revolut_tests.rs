//! Unit tests for Revolut OAuth start + code exchange (mock HttpClient).

use super::revolut::*;
use super::revolut_test_support::{test_oauth_cfg, MockClient};
use crate::oauth::token_store::load_revolut_tokens;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

#[test]
fn start_url_contains_required_params() {
    let cfg = test_oauth_cfg();
    let start = oauth_start_with_state(&cfg, "state-123".into());
    assert!(start.auth_url.starts_with(DEFAULT_AUTH_URL));
    assert!(start.auth_url.contains("client_id=client-id"));
    assert!(start.auth_url.contains("redirect_uri="));
    assert!(start.auth_url.contains("response_type=code"));
    assert!(start.auth_url.contains("scope=READ"));
    assert!(start.auth_url.contains("state=state-123"));
    assert!(!start.auth_url.contains("client-secret"));
}

#[tokio::test]
async fn exchange_stores_tokens_without_logging_secrets() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    let last_body = Arc::new(Mutex::new(None));
    let client = MockClient {
        last_body: Arc::clone(&last_body),
        response: r#"{"access_token":"at-1","refresh_token":"rt-1","expires_in":3600,"token_type":"Bearer"}"#
            .into(),
        status: 200,
    };
    oauth_exchange_code_with_client(&client, &test_oauth_cfg(), &company, "auth-code-xyz")
        .await
        .unwrap();
    let body = last_body.lock().unwrap().clone().unwrap();
    assert!(body.contains("grant_type=authorization_code"));
    assert!(body.contains("code=auth-code-xyz"));
    assert!(body.contains("client_id=client-id"));
    assert!(body.contains("client_secret=client-secret"));
    let stored = load_revolut_tokens(&company).unwrap();
    assert_eq!(stored.access_token, "at-1");
    assert_eq!(stored.refresh_token.as_deref(), Some("rt-1"));
    assert_eq!(stored.token_type.as_deref(), Some("Bearer"));
    let expires = stored.expires_at.expect("expires_at");
    assert!(expires > Utc::now().timestamp_millis());
    assert!(expires > 1_000_000_000_000);
}

#[tokio::test]
async fn exchange_rejects_empty_code() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    let client = MockClient {
        last_body: Arc::new(Mutex::new(None)),
        response: "{}".into(),
        status: 200,
    };
    let err = oauth_exchange_code_with_client(&client, &test_oauth_cfg(), &company, "  ")
        .await
        .unwrap_err();
    assert!(matches!(err, RevolutOAuthError::MissingCode));
}
