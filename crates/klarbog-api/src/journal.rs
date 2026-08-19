//! Journal preview/commit HTTP handlers.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{journal_commit, journal_preview, CoreError};
use klarbog_journal::JournalEntry;
use klarbog_types::{Envelope, KlarbogError};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct JournalPreviewBody {
    pub company: String,
    pub entry: JournalEntry,
}

#[derive(Deserialize)]
pub struct JournalCommitBody {
    pub company: String,
    pub entry: JournalEntry,
    pub confirm_token: String,
}

fn map_core_error(err: CoreError) -> (StatusCode, Envelope<Value>) {
    match err {
        CoreError::ActorDenied(tag) => (
            StatusCode::FORBIDDEN,
            Envelope::err([format!("actor not in policy: {tag}")]),
        ),
        CoreError::Path(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::Journal(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::RulesViolation(msg) => (StatusCode::BAD_REQUEST, Envelope::err([msg])),
        CoreError::Confirm(KlarbogError::ConfirmRequired) => (
            StatusCode::BAD_REQUEST,
            Envelope::err(["confirm token required or already consumed"]),
        ),
        CoreError::Confirm(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::Store(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        CoreError::Other(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<JournalPreviewBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let result = journal_preview(
        &state.allowlist_root,
        &company,
        &body.entry,
        &actor,
        &state.confirm,
        &state.registry,
    )
    .await
    .map_err(|e| {
        let (status, env) = map_core_error(e);
        (status, Json(env))
    })?;
    Ok(Json(Envelope::ok_with_rules(
        serde_json::json!({
            "confirm_token": result.confirm_token.token,
            "expires_unix_ms": result.confirm_token.expires_unix_ms,
            "payload_digest": result.payload_digest,
        }),
        result.applied_rules,
    )))
}

pub async fn commit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<JournalCommitBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let result = journal_commit(
        &state.allowlist_root,
        &company,
        body.entry,
        &actor,
        &body.confirm_token,
        &state.confirm,
        &state.registry,
    )
    .await
    .map_err(|e| {
        let (status, env) = map_core_error(e);
        (status, Json(env))
    })?;
    Ok(Json(Envelope::ok_with_rules(
        serde_json::json!({
            "id": result.posted.id,
            "digest": result.posted.digest,
            "prev_digest": result.posted.prev_digest,
        }),
        result.applied_rules,
    )))
}
