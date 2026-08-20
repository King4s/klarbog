//! Unit tests for Revolut OAuth refresh + resolve (mock HttpClient).

use super::revolut::*;
use super::revolut_test_support::{
    seed_tokens, set_oauth_env, test_oauth_cfg, EnvGuard, MockClient,
};
use crate::oauth::token_store::load_revolut_tokens;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

#[tokio::test]
async fn refresh_rotates_secrets_atomically() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    seed_tokens(&company, Some("rt-old"), Some(1));
    let last_body = Arc::new(Mutex::new(None));
    let client = MockClient {
        last_body: Arc::clone(&last_body),
        response: r#"{"access_token":"at-new","refresh_token":"rt-new","expires_in":7200,"token_type":"Bearer"}"#
            .into(),
        status: 200,
    };
    let out = refresh_access_token_with_client(&client, &test_oauth_cfg(), &company)
        .await
        .unwrap();
    assert_eq!(out.access_token, "at-new");
    assert_eq!(out.refresh_token.as_deref(), Some("rt-new"));
    let body = last_body.lock().unwrap().clone().unwrap();
    assert!(body.contains("grant_type=refresh_token"));
    assert!(body.contains("refresh_token=rt-old"));
    assert!(!body.contains("authorization_code"));
    let stored = load_revolut_tokens(&company).unwrap();
    assert_eq!(stored.access_token, "at-new");
    assert_eq!(stored.refresh_token.as_deref(), Some("rt-new"));
}

#[tokio::test]
async fn refresh_keeps_old_refresh_when_response_omits_it() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    seed_tokens(&company, Some("rt-keep"), Some(1));
    let client = MockClient {
        last_body: Arc::new(Mutex::new(None)),
        response: r#"{"access_token":"at-2","expires_in":60}"#.into(),
        status: 200,
    };
    let out = refresh_access_token_with_client(&client, &test_oauth_cfg(), &company)
        .await
        .unwrap();
    assert_eq!(out.refresh_token.as_deref(), Some("rt-keep"));
}

#[tokio::test]
async fn refresh_fail_closed_without_refresh_token() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    seed_tokens(&company, None, Some(1));
    let client = MockClient {
        last_body: Arc::new(Mutex::new(None)),
        response: "{}".into(),
        status: 200,
    };
    let err = refresh_access_token_with_client(&client, &test_oauth_cfg(), &company)
        .await
        .unwrap_err();
    assert!(matches!(err, RevolutOAuthError::MissingRefresh));
    assert!(client.last_body.lock().unwrap().is_none());
}

#[tokio::test]
async fn refresh_fail_closed_on_blank_refresh_token() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    seed_tokens(&company, Some("   "), Some(1));
    let client = MockClient {
        last_body: Arc::new(Mutex::new(None)),
        response: "{}".into(),
        status: 200,
    };
    let err = refresh_access_token_with_client(&client, &test_oauth_cfg(), &company)
        .await
        .unwrap_err();
    assert!(matches!(err, RevolutOAuthError::MissingRefresh));
    assert!(client.last_body.lock().unwrap().is_none());
}

#[tokio::test]
async fn refresh_provider_http_error_preserves_secrets() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    seed_tokens(&company, Some("rt-keep"), Some(1));
    let client = MockClient {
        last_body: Arc::new(Mutex::new(None)),
        response: r#"{"error":"invalid_grant","access_token":"should-not-store"}"#.into(),
        status: 401,
    };
    let err = refresh_access_token_with_client(&client, &test_oauth_cfg(), &company)
        .await
        .unwrap_err();
    assert!(matches!(err, RevolutOAuthError::Refresh(_)));
    let msg = err.to_string();
    assert!(!msg.contains("rt-keep"));
    assert!(!msg.contains("old-access"));
    assert!(!msg.contains("should-not-store"));
    let stored = load_revolut_tokens(&company).unwrap();
    assert_eq!(stored.access_token, "old-access");
    assert_eq!(stored.refresh_token.as_deref(), Some("rt-keep"));
}

#[tokio::test]
async fn resolve_fail_closed_when_expired_without_refresh() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    seed_tokens(&company, None, Some(1));
    let _token = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
    let _oauth = set_oauth_env();
    let client = MockClient {
        last_body: Arc::new(Mutex::new(None)),
        response: r#"{"access_token":"at-leak","refresh_token":"rt-leak","expires_in":60}"#.into(),
        status: 200,
    };
    let err = from_env_or_company_secrets_refreshed(&client, &company)
        .await
        .unwrap_err();
    assert!(matches!(err, RevolutOAuthError::MissingRefresh));
    assert!(client.last_body.lock().unwrap().is_none());
    let stored = load_revolut_tokens(&company).unwrap();
    assert_eq!(stored.access_token, "old-access");
    assert!(stored.refresh_token.is_none());
}

#[tokio::test]
async fn resolve_refreshes_when_access_expired() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    seed_tokens(&company, Some("rt-1"), Some(1));
    let _token = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
    let _oauth = set_oauth_env();
    let client = MockClient {
        last_body: Arc::new(Mutex::new(None)),
        response: r#"{"access_token":"at-fresh","refresh_token":"rt-1","expires_in":3600}"#.into(),
        status: 200,
    };
    let cfg = from_env_or_company_secrets_refreshed(&client, &company)
        .await
        .unwrap();
    assert_eq!(cfg.token, "at-fresh");
}

#[tokio::test]
async fn resolve_skips_network_when_access_valid() {
    let dir = tempdir().unwrap();
    let company = dir.path().join("co");
    std::fs::create_dir_all(&company).unwrap();
    let future = Utc::now().timestamp_millis() + 3_600_000;
    seed_tokens(&company, Some("rt-1"), Some(future));
    let _token = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
    let last_body = Arc::new(Mutex::new(None));
    let client = MockClient {
        last_body: Arc::clone(&last_body),
        response: "{}".into(),
        status: 200,
    };
    let cfg = from_env_or_company_secrets_refreshed(&client, &company)
        .await
        .unwrap();
    assert_eq!(cfg.token, "old-access");
    assert!(last_body.lock().unwrap().is_none());
}
