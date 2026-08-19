//! Wave2 bank MCP tools — stripe consume, reconcile apply, Revolut OAuth refresh.

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use chrono::{DateTime, NaiveDate, Utc};
use klarbog_plugin_bank::{
    apply_match, consume_stripe_webhook_queue, refresh_access_token, BankRow, ConsumeOpts,
    ReconcileError, RevolutOAuthConfig, RevolutOAuthError, StripeWebhookError,
};
use klarbog_types::{Envelope, MinorAmount};
use serde_json::{json, Value};
use std::path::Path;

fn map_webhook(err: StripeWebhookError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn map_reconcile(err: ReconcileError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn map_oauth(err: RevolutOAuthError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn parse_row_fields(args: &Value) -> Result<(BankRow, usize), String> {
    let row = args.get("row").ok_or_else(|| "missing row".to_string())?;
    let date_raw = row
        .get("date")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing row.date".to_string())?;
    let text = row
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing row.text".to_string())?
        .to_string();
    let amount_minor = row
        .get("amount_minor")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "missing row.amount_minor".to_string())?;
    let date = NaiveDate::parse_from_str(date_raw, "%Y-%m-%d")
        .map_err(|e| format!("invalid date {date_raw}: {e}"))?;
    let dt: DateTime<Utc> = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let row_index = args
        .get("row_index")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(0);
    Ok((
        BankRow {
            date: dt,
            text,
            amount_minor: MinorAmount::from_minor(amount_minor),
        },
        row_index,
    ))
}

/// Mirror POST /api/v1/bank/stripe/consume — fail-closed unless confirm:true.
pub async fn bank_stripe_consume(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let confirm = args
        .get("confirm")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let dry_run = !confirm;
    match consume_stripe_webhook_queue(&path, ConsumeOpts { dry_run, limit }) {
        Ok(report) => Envelope::ok(serde_json::to_value(report).unwrap()),
        Err(e) => map_webhook(e),
    }
}

/// Mirror POST /api/v1/bank/reconcile/apply — journal preview only (no post).
pub async fn bank_reconcile_apply(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let invoice_id = match args.get("invoice_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return Envelope::err(["missing invoice_id"]),
    };
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let (row, row_index) = match parse_row_fields(args) {
        Ok(r) => r,
        Err(e) => return Envelope::err([e]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match apply_match(&path, &row, &invoice_id, &actor, force, row_index) {
        Ok(applied) => Envelope::ok(json!({
            "entry": applied.entry,
            "confidence_bps": applied.confidence_bps,
            "forced": applied.forced,
            "exception_closed": applied.exception_closed.as_ref().map(|e| e.id.to_string()),
            "invoice_id": invoice_id,
        })),
        Err(e) => map_reconcile(e),
    }
}

/// Mirror POST /api/v1/revolut/oauth/refresh — never echo tokens.
pub async fn revolut_oauth_refresh(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let cfg = match RevolutOAuthConfig::from_env() {
        Ok(c) => c,
        Err(e) => return map_oauth(RevolutOAuthError::Config(e)),
    };
    match refresh_access_token(&cfg, &path).await {
        Ok(_) => Envelope::ok(json!({
            "refreshed": true,
            "path": "secrets/revolut.json",
        })),
        Err(e) => map_oauth(e),
    }
}

#[cfg(test)]
#[path = "bank_wave2_tests.rs"]
mod tests;
