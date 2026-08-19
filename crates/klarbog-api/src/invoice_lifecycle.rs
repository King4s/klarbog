//! Invoice lifecycle HTTP: PATCH status, POST mark-paid / mark-part-paid preview.

use crate::actor::parse_actor;
use crate::invoice::{authorize_company, map_core, map_invoice};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_invoice::{
    mark_paid_preview, mark_part_paid_preview, patch_status, InvoiceConfig, InvoiceId,
    InvoiceStatus,
};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct PatchStatusBody {
    pub company: String,
    pub invoice_id: String,
    pub status: InvoiceStatus,
}

#[derive(Deserialize)]
pub struct MarkPaidBody {
    pub company: String,
    pub invoice_id: String,
}

#[derive(Deserialize)]
pub struct MarkPartPaidBody {
    pub company: String,
    pub invoice_id: String,
    pub amount_minor: i64,
}

pub async fn patch_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PatchStatusBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = InvoiceId::new(body.invoice_id);
    let invoice = patch_status(&path, &id, body.status).map_err(|e| {
        let (s, env) = map_invoice(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(invoice).unwrap())))
}

pub async fn mark_paid(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MarkPaidBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = InvoiceId::new(body.invoice_id);
    let (invoice, journal_entry) = mark_paid_preview(&path, &id, &actor, &InvoiceConfig::default())
        .map_err(|e| {
            let (s, env) = map_invoice(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "invoice": invoice,
        "journal_entry": journal_entry,
    }))))
}

pub async fn mark_part_paid(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MarkPartPaidBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = InvoiceId::new(body.invoice_id);
    let (invoice, journal_entry) = mark_part_paid_preview(
        &path,
        &id,
        body.amount_minor,
        &actor,
        &InvoiceConfig::default(),
    )
    .map_err(|e| {
        let (s, env) = map_invoice(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "invoice": invoice,
        "journal_entry": journal_entry,
    }))))
}
