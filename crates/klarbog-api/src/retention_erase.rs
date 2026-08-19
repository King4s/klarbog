//! GDPR party erase HTTP handler (slice 25).

use crate::actor::parse_actor;
use crate::retention::{authorize_company, map_core, map_gdpr};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_retention::{erase_party, ErasePartyOptions};
use klarbog_types::{Envelope, PartyId};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct ErasePartyBody {
    pub company: String,
    pub party_id: String,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default)]
    pub delete_documents: bool,
}

pub async fn post_erase_party(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ErasePartyBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let party_id = PartyId::new(body.party_id);
    let report = erase_party(
        &path,
        &party_id,
        ErasePartyOptions {
            confirm: body.confirm,
            delete_documents: body.delete_documents,
        },
    )
    .await
    .map_err(|e| {
        let (s, env) = map_gdpr(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(report).unwrap())))
}
