//! POST /api/v1/bank/stripe/reconcile-suggest — consume + suggest (no journal post).

use crate::actor::parse_actor;
use crate::bank::{authorize_company, map_core};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_bank::{
    suggest_from_stripe_consume_with, ConsumeOpts, ReconcileError, StripeReconcilePipelineError,
    StripeWebhookError,
};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct StripeReconcileSuggestBody {
    pub company: String,
    /// When true, persist `queue.consumed` then suggest. Default false = dry-run consume.
    #[serde(default)]
    pub confirm_consume: bool,
    pub limit: Option<usize>,
    #[serde(default = "default_raise_exceptions")]
    pub raise_exceptions: bool,
}

fn default_raise_exceptions() -> bool {
    true
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
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Envelope::err([err.to_string()]),
    )
}

pub async fn reconcile_suggest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<StripeReconcileSuggestBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let dry_run = !body.confirm_consume;
    let report = suggest_from_stripe_consume_with(
        &path,
        ConsumeOpts {
            dry_run,
            limit: body.limit,
        },
        body.raise_exceptions,
    )
    .map_err(|e| {
        let (s, env) = map_pipeline(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(report).unwrap())))
}
