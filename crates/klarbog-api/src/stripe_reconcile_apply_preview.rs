//! POST /api/v1/bank/stripe/reconcile-apply-preview — consume → unique safe apply + ConfirmStore.
//! Still **no** journal commit.

use crate::actor::parse_actor;
use crate::bank::{authorize_company, map_core};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::journal_preview;
use klarbog_plugin_bank::{
    apply_preview_from_stripe_consume_with, ConsumeOpts, ReconcileError,
    StripeReconcilePipelineError, StripeWebhookError,
};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct StripeReconcileApplyPreviewBody {
    pub company: String,
    /// When true, persist `queue.consumed` then suggest/apply. Default false = dry-run consume.
    #[serde(default)]
    pub confirm_consume: bool,
    #[serde(default)]
    pub force: bool,
    pub limit: Option<usize>,
}

fn map_pipeline(err: StripeReconcilePipelineError) -> (StatusCode, Envelope<Value>) {
    match err {
        StripeReconcilePipelineError::Webhook(e) => map_webhook(e),
        StripeReconcilePipelineError::Reconcile(e) => map_reconcile(e),
    }
}

fn map_webhook(err: StripeWebhookError) -> (StatusCode, Envelope<Value>) {
    let status = match &err {
        StripeWebhookError::InvalidJson(_) | StripeWebhookError::MissingField(_) => {
            StatusCode::BAD_REQUEST
        }
        StripeWebhookError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Envelope::err([err.to_string()]))
}

fn map_reconcile(err: ReconcileError) -> (StatusCode, Envelope<Value>) {
    let status = match &err {
        ReconcileError::InvoiceNotFound(_) => StatusCode::NOT_FOUND,
        ReconcileError::InvoiceNotOpen(_)
        | ReconcileError::AmountMismatch
        | ReconcileError::UnsafeMatch { .. } => StatusCode::BAD_REQUEST,
        ReconcileError::ForceRequiresUser => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Envelope::err([err.to_string()]))
}

pub async fn reconcile_apply_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<StripeReconcileApplyPreviewBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company.clone());
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let dry_run = !body.confirm_consume;
    let report = apply_preview_from_stripe_consume_with(
        &path,
        ConsumeOpts {
            dry_run,
            limit: body.limit,
        },
        &actor,
        body.force,
        true,
    )
    .map_err(|e| {
        let (s, env) = map_pipeline(e);
        (s, Json(env))
    })?;
    let mut data = serde_json::to_value(&report).unwrap();
    let Some(applied) = report.applied.as_ref() else {
        return Ok(Json(Envelope::ok(data)));
    };
    let preview = journal_preview(
        &state.allowlist_root,
        &company,
        &applied.result.entry,
        &actor,
        &state.confirm,
        &state.registry,
    )
    .await
    .map_err(|e| {
        let (s, env) = map_core(e);
        (s, Json(env))
    })?;
    if let Some(obj) = data.get_mut("applied").and_then(|v| v.as_object_mut()) {
        obj.insert(
            "confirm_token".into(),
            Value::String(preview.confirm_token.token.clone()),
        );
        obj.insert(
            "expires_unix_ms".into(),
            serde_json::json!(preview.confirm_token.expires_unix_ms),
        );
        obj.insert(
            "payload_digest".into(),
            Value::String(preview.payload_digest),
        );
    }
    Ok(Json(Envelope::ok_with_rules(data, preview.applied_rules)))
}
