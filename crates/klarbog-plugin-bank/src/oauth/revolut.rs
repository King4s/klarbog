//! Revolut OAuth scaffold — auth URL + authorization-code exchange (ADR-008).

use crate::api::{BankApiError, HttpClient, ReqwestHttpClient};
use crate::config::{BankApiConfigError, RevolutOAuthConfig};
use crate::oauth::token_store::{save_revolut_tokens, RevolutStoredTokens};
use chrono::Utc;
use serde::Deserialize;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_AUTH_URL: &str = "https://business.revolut.com/app-confirm";
pub const DEFAULT_SCOPE: &str = "READ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevolutOAuthStart {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Error)]
pub enum RevolutOAuthError {
    #[error(transparent)]
    Config(#[from] BankApiConfigError),
    #[error("missing authorization code")]
    MissingCode,
    #[error("token exchange failed: {0}")]
    Exchange(String),
    #[error(transparent)]
    Api(#[from] BankApiError),
    #[error(transparent)]
    Store(#[from] crate::oauth::token_store::RevolutTokenStoreError),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    token_type: Option<String>,
}

pub fn oauth_start(cfg: &RevolutOAuthConfig) -> RevolutOAuthStart {
    oauth_start_with_state(cfg, Uuid::new_v4().to_string())
}

pub fn oauth_start_with_state(cfg: &RevolutOAuthConfig, state: String) -> RevolutOAuthStart {
    let auth_url = format!(
        "{}?{}",
        cfg.auth_url.trim_end_matches('?'),
        form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &cfg.client_id)
            .append_pair("redirect_uri", &cfg.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", DEFAULT_SCOPE)
            .append_pair("state", &state)
            .finish()
    );
    RevolutOAuthStart { auth_url, state }
}

pub async fn oauth_exchange_code(
    cfg: &RevolutOAuthConfig,
    company: &Path,
    code: &str,
) -> Result<(), RevolutOAuthError> {
    oauth_exchange_code_with_client(&ReqwestHttpClient, cfg, company, code).await
}

pub async fn oauth_exchange_code_with_client<C: HttpClient>(
    client: &C,
    cfg: &RevolutOAuthConfig,
    company: &Path,
    code: &str,
) -> Result<(), RevolutOAuthError> {
    if code.trim().is_empty() {
        return Err(RevolutOAuthError::MissingCode);
    }
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", code.trim())
        .append_pair("client_id", &cfg.client_id)
        .append_pair("client_secret", &cfg.client_secret)
        .append_pair("redirect_uri", &cfg.redirect_uri)
        .finish();
    let (status, resp_body) = client.post_form(&cfg.token_url, &body, None).await?;
    if status != 200 {
        return Err(RevolutOAuthError::Exchange(format!(
            "http {status}: token response rejected"
        )));
    }
    let parsed: TokenResponse = serde_json::from_str(&resp_body)
        .map_err(|e| RevolutOAuthError::Exchange(format!("invalid token json: {e}")))?;
    if parsed.access_token.trim().is_empty() {
        return Err(RevolutOAuthError::Exchange(
            "empty access_token in response".into(),
        ));
    }
    let expires_at = parsed.expires_in.map(|secs| Utc::now().timestamp() + secs);
    let stored = RevolutStoredTokens {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        expires_at,
        token_type: parsed.token_type,
    };
    save_revolut_tokens(company, &stored)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::HttpClient;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    struct MockClient {
        last_body: Arc<Mutex<Option<String>>>,
        response: String,
    }

    impl HttpClient for MockClient {
        async fn get(
            &self,
            _url: &str,
            _bearer_token: &str,
        ) -> Result<(u16, String), BankApiError> {
            Ok((200, "[]".into()))
        }

        async fn post_form(
            &self,
            _url: &str,
            body: &str,
            _bearer_token: Option<&str>,
        ) -> Result<(u16, String), BankApiError> {
            *self.last_body.lock().unwrap() = Some(body.to_string());
            Ok((200, self.response.clone()))
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
        };
        oauth_exchange_code_with_client(&client, &test_oauth_cfg(), &company, "auth-code-xyz")
            .await
            .unwrap();
        let body = last_body.lock().unwrap().clone().unwrap();
        assert!(body.contains("grant_type=authorization_code"));
        assert!(body.contains("code=auth-code-xyz"));
        assert!(body.contains("client_id=client-id"));
        assert!(body.contains("client_secret=client-secret"));
        let stored = crate::oauth::token_store::load_revolut_tokens(&company).unwrap();
        assert_eq!(stored.access_token, "at-1");
        assert_eq!(stored.refresh_token.as_deref(), Some("rt-1"));
        assert_eq!(stored.token_type.as_deref(), Some("Bearer"));
        assert!(stored.expires_at.is_some());
    }

    #[tokio::test]
    async fn exchange_rejects_empty_code() {
        let dir = tempdir().unwrap();
        let company = dir.path().join("co");
        std::fs::create_dir_all(&company).unwrap();
        let client = MockClient {
            last_body: Arc::new(Mutex::new(None)),
            response: "{}".into(),
        };
        let err = oauth_exchange_code_with_client(&client, &test_oauth_cfg(), &company, "  ")
            .await
            .unwrap_err();
        assert!(matches!(err, RevolutOAuthError::MissingCode));
    }
}
