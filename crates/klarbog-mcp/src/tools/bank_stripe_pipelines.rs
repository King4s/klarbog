//! MCP parity for Stripe consume → reconcile suggest / apply-preview pipelines.

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_core::{journal_preview, ConfirmStore};
use klarbog_plugin::Registry;
use klarbog_plugin_bank::{
    apply_preview_from_stripe_consume_with, suggest_from_stripe_consume_with, ConsumeOpts,
    ReconcileError, StripeReconcilePipelineError, StripeWebhookError,
};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

fn map_pipeline(err: StripeReconcilePipelineError) -> Envelope<Value> {
    match err {
        StripeReconcilePipelineError::Webhook(e) => map_webhook(e),
        StripeReconcilePipelineError::Reconcile(e) => map_reconcile(e),
    }
}

fn map_webhook(err: StripeWebhookError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn map_reconcile(err: ReconcileError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

/// Mirror POST /api/v1/bank/stripe/reconcile-suggest — dry-run consume unless confirm_consume.
pub async fn bank_stripe_reconcile_suggest(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let confirm_consume = args
        .get("confirm_consume")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let raise_exceptions = args
        .get("raise_exceptions")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match suggest_from_stripe_consume_with(
        &path,
        ConsumeOpts {
            dry_run: !confirm_consume,
            limit,
        },
        raise_exceptions,
    ) {
        Ok(report) => Envelope::ok(serde_json::to_value(report).unwrap()),
        Err(e) => map_pipeline(e),
    }
}

/// Mirror POST /api/v1/bank/stripe/reconcile-apply-preview — unique safe → ConfirmStore; no commit.
pub async fn bank_stripe_reconcile_apply_preview(
    args: &Value,
    allowlist_root: &Path,
    store: &ConfirmStore,
    registry: &Registry,
) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let confirm_consume = args
        .get("confirm_consume")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let report = match apply_preview_from_stripe_consume_with(
        &path,
        ConsumeOpts {
            dry_run: !confirm_consume,
            limit,
        },
        &actor,
        force,
        true,
    ) {
        Ok(r) => r,
        Err(e) => return map_pipeline(e),
    };
    let mut data = serde_json::to_value(&report).unwrap();
    let Some(applied) = report.applied.as_ref() else {
        return Envelope::ok(data);
    };
    let preview = match journal_preview(
        allowlist_root,
        &company,
        &applied.result.entry,
        &actor,
        store,
        registry,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    if let Some(obj) = data.get_mut("applied").and_then(|v| v.as_object_mut()) {
        obj.insert(
            "confirm_token".into(),
            Value::String(preview.confirm_token.token.clone()),
        );
        obj.insert(
            "expires_unix_ms".into(),
            json!(preview.confirm_token.expires_unix_ms),
        );
        obj.insert(
            "payload_digest".into(),
            Value::String(preview.payload_digest),
        );
    }
    Envelope::ok_with_rules(data, preview.applied_rules)
}

#[cfg(test)]
#[path = "bank_stripe_pipelines_tests.rs"]
mod tests;
