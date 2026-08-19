//! Revolut OAuth HTTP scaffold (ADR-008 — no full UI).

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_bank::{
    oauth_exchange_code, oauth_start, refresh_access_token, RevolutOAuthConfig, RevolutOAuthError,
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

#[derive(Deserialize)]
pub struct OAuthRefreshBody {
    pub company: String,
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
        RevolutOAuthError::MissingCode | RevolutOAuthError::MissingRefresh => {
            StatusCode::BAD_REQUEST
        }
        RevolutOAuthError::Exchange(_)
        | RevolutOAuthError::Refresh(_)
        | RevolutOAuthError::Api(_) => StatusCode::BAD_GATEWAY,
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

pub async fn oauth_refresh_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OAuthRefreshBody>,
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
    refresh_access_token(&cfg, &path).await.map_err(|e| {
        let (s, env) = map_oauth(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "refreshed": true,
        "path": "secrets/revolut.json",
    }))))
}
