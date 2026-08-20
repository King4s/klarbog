//! Unit tests for Revolut OAuth exchange + refresh (mock HttpClient).

use super::revolut::*;
use crate::api::{BankApiError, HttpClient};
use crate::config::RevolutOAuthConfig;
use crate::oauth::token_store::{load_revolut_tokens, save_revolut_tokens, RevolutStoredTokens};
use chrono::Utc;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

struct MockClient {
    last_body: Arc<Mutex<Option<String>>>,
    response: String,
    status: u16,
}

impl HttpClient for MockClient {
    async fn get(&self, _url: &str, _bearer_token: &str) -> Result<(u16, String), BankApiError> {
        Ok((200, "[]".into()))
    }

    async fn post_form(
        &self,
        _url: &str,
        body: &str,
        _bearer_token: Option<&str>,
    ) -> Result<(u16, String), BankApiError> {
        *self.last_body.lock().unwrap() = Some(body.to_string());
        Ok((self.status, self.response.clone()))
    }
}

fn test_oauth_cfg() -> RevolutOAuthConfig {
    RevolutOAuthConfig {
        client_id: "client-id".into(),
        client_secret: "client-secret".into(),
        redirect_uri: "https://example.test/callback".into(),
        token_url: "https://example.test/token".into(),
        auth_url: DEFAULT_AUTH_URL.into(),
    }
}

fn seed_tokens(company: &Path, refresh: Option<&str>, expires_at: Option<i64>) {
    save_revolut_tokens(
        company,
        &RevolutStoredTokens {
            access_token: "old-access".into(),
            refresh_token: refresh.map(str::to_string),
            expires_at,
            token_type: Some("Bearer".into()),
        },
    )
    .unwrap();
}

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
