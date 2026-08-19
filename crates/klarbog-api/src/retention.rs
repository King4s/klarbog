//! Retention, backup manifest, GDPR export HTTP handlers. AuthZ mirrors CRM/documents.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_retention::{
    load_retention, run_retention_purge, write_backup_manifest, write_gdpr_export, BackupError,
    GdprError, PurgeError, PurgeOptions, RetentionError,
};
use klarbog_types::{Actor, Envelope};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct RetentionQuery {
    pub company: String,
}

#[derive(Deserialize)]
pub struct CompanyBody {
    pub company: String,
}

#[derive(Deserialize)]
pub struct PurgeBody {
    pub company: String,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default)]
    pub gc_orphan_documents: bool,
}

pub(crate) async fn authorize_company(
    allowlist_root: &Path,
    company: &Path,
    actor: &Actor,
) -> Result<PathBuf, CoreError> {
    let path = assert_company_path(allowlist_root, company)?;
    open_existing(&path).await?.authorize(actor)?;
    Ok(path)
}

pub(crate) fn map_core(err: CoreError) -> (StatusCode, Envelope<Value>) {
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

fn map_retention(err: RetentionError) -> (StatusCode, Envelope<Value>) {
    match err {
        RetentionError::InvalidRetainDays => {
            (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
        }
        RetentionError::Io(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        RetentionError::Json(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

fn map_backup(err: BackupError) -> (StatusCode, Envelope<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Envelope::err([err.to_string()]),
    )
}

pub(crate) fn map_gdpr(err: GdprError) -> (StatusCode, Envelope<Value>) {
    match err {
        GdprError::PartyNotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("party not found: {id}")]),
        ),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([other.to_string()]),
        ),
    }
}

fn map_purge(err: PurgeError) -> (StatusCode, Envelope<Value>) {
    match err {
        PurgeError::Retention(e) => map_retention(e),
        PurgeError::Documents(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

pub async fn get_retention(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RetentionQuery>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(query.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let policy = load_retention(&path).map_err(|e| {
        let (s, env) = map_retention(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(policy).unwrap())))
}

pub async fn post_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CompanyBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let manifest = write_backup_manifest(&path).await.map_err(|e| {
        let (s, env) = map_backup(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(manifest).unwrap())))
}

pub async fn post_gdpr_export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CompanyBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let export = write_gdpr_export(&path).map_err(|e| {
        let (s, env) = map_gdpr(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(export).unwrap())))
}

pub async fn post_purge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PurgeBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let report = run_retention_purge(
        &path,
        PurgeOptions {
            confirm: body.confirm,
            gc_orphan_documents: body.gc_orphan_documents,
        },
    )
    .await
    .map_err(|e| {
        let (s, env) = map_purge(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(report).unwrap())))
}
