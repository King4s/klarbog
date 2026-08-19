//! Exception HTTP handlers (pair with `documents`).

use crate::actor::parse_actor;
use crate::documents::{authorize_company, map_core, map_doc};
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_documents::{
    get_exception, list_exceptions, raise_exception, set_exception_open, ExceptionId,
    ExceptionSeverity,
};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct RaiseBody {
    pub company: String,
    pub code: String,
    pub severity: ExceptionSeverity,
    pub message: String,
    #[serde(default)]
    pub related_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct ExceptionListQuery {
    pub company: String,
    pub exception_id: Option<String>,
    #[serde(default = "default_open_only")]
    pub open_only: bool,
}

fn default_open_only() -> bool {
    true
}

#[derive(Deserialize)]
pub struct CloseBody {
    pub company: String,
    pub exception_id: String,
    #[serde(default)]
    pub open: bool,
}

pub async fn raise(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RaiseBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let exc = raise_exception(
        &path,
        body.code,
        body.severity,
        body.message,
        body.related_ids,
    )
    .map_err(|e| {
        let (s, env) = map_doc(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(exc).unwrap())))
}

pub async fn list_exc(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ExceptionListQuery>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(query.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    if let Some(raw_id) = query.exception_id {
        let id = ExceptionId::new(raw_id);
        let exc = get_exception(&path, &id).map_err(|e| {
            let (s, env) = map_doc(e);
            (s, Json(env))
        })?;
        let exc = exc.ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(Envelope::err([format!("exception not found: {id}")])),
            )
        })?;
        return Ok(Json(Envelope::ok(serde_json::to_value(exc).unwrap())));
    }
    let items = list_exceptions(&path, query.open_only).map_err(|e| {
        let (s, env) = map_doc(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(items).unwrap())))
}

pub async fn close_exc(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CloseBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = ExceptionId::new(body.exception_id);
    let exc = set_exception_open(&path, &id, body.open).map_err(|e| {
        let (s, env) = map_doc(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(exc).unwrap())))
}
