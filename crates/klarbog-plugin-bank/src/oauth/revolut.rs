//! Revolut OAuth scaffold — auth URL, code exchange, refresh (ADR-008).

use crate::api::{BankApiError, HttpClient, ReqwestHttpClient};
use crate::config::{RevolutApiConfig, RevolutOAuthConfig};
use crate::oauth::token_store::{
    access_token_expired, load_revolut_tokens, save_revolut_tokens, RevolutStoredTokens,
};
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
    Config(#[from] crate::config::BankApiConfigError),
    #[error("missing authorization code")]
    MissingCode,
    #[error("missing refresh_token in company secrets")]
    MissingRefresh,
    #[error("token exchange failed: {0}")]
    Exchange(String),
    #[error("token refresh failed: {0}")]
    Refresh(String),
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
    let stored = token_grant_with_client(client, cfg, &body, "exchange").await?;
    save_revolut_tokens(company, &stored)?;
    Ok(())
}

pub async fn refresh_access_token(
    cfg: &RevolutOAuthConfig,
    company: &Path,
) -> Result<RevolutStoredTokens, RevolutOAuthError> {
    refresh_access_token_with_client(&ReqwestHttpClient, cfg, company).await
}

pub async fn refresh_access_token_with_client<C: HttpClient>(
    client: &C,
    cfg: &RevolutOAuthConfig,
    company: &Path,
) -> Result<RevolutStoredTokens, RevolutOAuthError> {
    let existing = load_revolut_tokens(company)?;
    let refresh = existing
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(RevolutOAuthError::MissingRefresh)?;
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("refresh_token", refresh)
        .append_pair("client_id", &cfg.client_id)
        .append_pair("client_secret", &cfg.client_secret)
        .finish();
    let mut stored = token_grant_with_client(client, cfg, &body, "refresh").await?;
    if stored.refresh_token.is_none() {
        stored.refresh_token = existing.refresh_token;
    }
    if stored.token_type.is_none() {
        stored.token_type = existing.token_type;
    }
    save_revolut_tokens(company, &stored)?;
    Ok(stored)
}

/// Env bearer first; else company secrets, refreshing when access is expired.
pub async fn from_env_or_company_secrets_refreshed<C: HttpClient>(
    client: &C,
    company: &Path,
) -> Result<RevolutApiConfig, RevolutOAuthError> {
    if let Ok(cfg) = RevolutApiConfig::from_env() {
        return Ok(cfg);
    }
    let tokens = load_revolut_tokens(company).map_err(|_| {
        RevolutOAuthError::Config(crate::config::BankApiConfigError::MissingEnv(
            "KLARBOG_REVOLUT_API_TOKEN",
        ))
    })?;
    let now_ms = Utc::now().timestamp_millis();
    if !access_token_expired(&tokens, now_ms) {
        return Ok(RevolutApiConfig {
            token: tokens.access_token,
            base_url: RevolutApiConfig::base_from_env(),
        });
    }
    let oauth = RevolutOAuthConfig::from_env()?;
    let refreshed = refresh_access_token_with_client(client, &oauth, company).await?;
    Ok(RevolutApiConfig {
        token: refreshed.access_token,
        base_url: RevolutApiConfig::base_from_env(),
    })
}

async fn token_grant_with_client<C: HttpClient>(
    client: &C,
    cfg: &RevolutOAuthConfig,
    body: &str,
    kind: &str,
) -> Result<RevolutStoredTokens, RevolutOAuthError> {
    let (status, resp_body) = client.post_form(&cfg.token_url, body, None).await?;
    if status != 200 {
        let msg = format!("http {status}: token response rejected");
        return Err(match kind {
            "refresh" => RevolutOAuthError::Refresh(msg),
            _ => RevolutOAuthError::Exchange(msg),
        });
    }
    let parsed: TokenResponse = serde_json::from_str(&resp_body).map_err(|e| {
        let msg = format!("invalid token json: {e}");
        match kind {
            "refresh" => RevolutOAuthError::Refresh(msg),
            _ => RevolutOAuthError::Exchange(msg),
        }
    })?;
    if parsed.access_token.trim().is_empty() {
        let msg = "empty access_token in response".into();
        return Err(match kind {
            "refresh" => RevolutOAuthError::Refresh(msg),
            _ => RevolutOAuthError::Exchange(msg),
        });
    }
    let expires_at = parsed
        .expires_in
        .map(|secs| Utc::now().timestamp_millis() + secs.saturating_mul(1000));
    Ok(RevolutStoredTokens {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        expires_at,
        token_type: parsed.token_type,
    })
}
