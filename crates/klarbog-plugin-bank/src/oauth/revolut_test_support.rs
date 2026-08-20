//! Shared fixtures for Revolut OAuth unit tests.

use crate::api::{BankApiError, HttpClient};
use crate::config::RevolutOAuthConfig;
use crate::oauth::revolut::DEFAULT_AUTH_URL;
use crate::oauth::token_store::{save_revolut_tokens, RevolutStoredTokens};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(super) struct MockClient {
    pub last_body: Arc<Mutex<Option<String>>>,
    pub response: String,
    pub status: u16,
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

pub(super) fn test_oauth_cfg() -> RevolutOAuthConfig {
    RevolutOAuthConfig {
        client_id: "client-id".into(),
        client_secret: "client-secret".into(),
        redirect_uri: "https://example.test/callback".into(),
        token_url: "https://example.test/token".into(),
        auth_url: DEFAULT_AUTH_URL.into(),
    }
}

pub(super) fn seed_tokens(company: &Path, refresh: Option<&str>, expires_at: Option<i64>) {
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

pub(super) fn set_oauth_env() -> Vec<EnvGuard> {
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

pub(super) struct EnvGuard {
    key: &'static str,
    prev: Option<String>,
}

impl EnvGuard {
    pub fn unset(key: &'static str) -> Self {
        let prev = std::env::var(key).ok();
        unsafe { std::env::remove_var(key) };
        Self { key, prev }
    }

    pub fn set(key: &'static str, value: &str) -> Self {
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
