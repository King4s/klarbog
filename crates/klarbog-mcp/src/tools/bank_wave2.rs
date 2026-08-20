//! Wave2 bank MCP tools — stripe consume, reconcile suggest/apply, Revolut OAuth refresh.

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use chrono::{DateTime, NaiveDate, Utc};
use klarbog_plugin_bank::{
    apply_match, consume_stripe_webhook_queue, default_source_for_rail, import_preview,
    list_unmatched_bank_exceptions, refresh_access_token, suggest_matches,
    sync_unmatched_exceptions, BankImportConfig, BankImportError, BankImportSource, BankProfile,
    BankRow, ConsumeOpts, ReconcileError, RevolutOAuthConfig, RevolutOAuthError,
    StripeWebhookError,
};
use klarbog_types::{Actor, Currency, Envelope, MinorAmount};
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

fn map_import(err: BankImportError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn parse_bank_row_object(row: &Value) -> Result<BankRow, String> {
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
    Ok(BankRow {
        date: dt,
        text,
        amount_minor: MinorAmount::from_minor(amount_minor),
    })
}

fn parse_row_fields(args: &Value) -> Result<(BankRow, usize), String> {
    let row = args.get("row").ok_or_else(|| "missing row".to_string())?;
    let bank_row = parse_bank_row_object(row)?;
    let row_index = args
        .get("row_index")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(0);
    Ok((bank_row, row_index))
}

async fn resolve_suggest_rows(
    company: &Path,
    args: &Value,
    actor: &Actor,
) -> Result<Vec<BankRow>, Envelope<Value>> {
    if let Some(rows_val) = args.get("rows") {
        let arr = rows_val
            .as_array()
            .ok_or_else(|| Envelope::err(["rows must be an array"]))?;
        let mut out = Vec::with_capacity(arr.len());
        for input in arr {
            out.push(parse_bank_row_object(input).map_err(|e| Envelope::err([e]))?);
        }
        return Ok(out);
    }
    let provider_raw = args
        .get("provider")
        .or_else(|| args.get("profile"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| Envelope::err(["missing provider or rows"]))?;
    let provider: BankProfile = serde_json::from_value(json!(provider_raw))
        .map_err(|e| Envelope::err([format!("invalid provider: {e}")]))?;
    let source = match args.get("source").and_then(|v| v.as_str()) {
        Some(raw) => serde_json::from_value(json!(raw))
            .map_err(|e| Envelope::err([format!("invalid source: {e}")]))?,
        None => default_source_for_rail(provider),
    };
    let csv = args.get("csv").and_then(|v| v.as_str());
    if matches!(source, BankImportSource::Csv) && csv.unwrap_or("").is_empty() {
        return Err(Envelope::err(["missing csv"]));
    }
    let currency_code = args
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("DKK");
    let currency = Currency::new(currency_code).map_err(|e| Envelope::err([e.to_string()]))?;
    let cfg = BankImportConfig {
        currency,
        ..BankImportConfig::default()
    };
    import_preview(source, provider, csv, &cfg, actor, Some(company))
        .await
        .map(|(rows, _)| rows)
        .map_err(map_import)
}

/// Mirror POST /api/v1/bank/reconcile/suggest — match bank rows to open invoices (no post).
pub async fn bank_reconcile_suggest(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let raise_exceptions = args
        .get("raise_exceptions")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let rows = match resolve_suggest_rows(&path, args, &actor).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    let match_results = match suggest_matches(&path, &rows) {
        Ok(r) => r,
        Err(e) => return map_reconcile(e),
    };
    let raised = if raise_exceptions {
        match sync_unmatched_exceptions(&path, &rows, &match_results) {
            Ok(r) => r,
            Err(e) => return map_reconcile(e),
        }
    } else {
        Vec::new()
    };
    let open_unmatched = match list_unmatched_bank_exceptions(&path) {
        Ok(r) => r,
        Err(e) => return map_reconcile(e),
    };
    Envelope::ok(json!({
        "count": match_results.len(),
        "rows": match_results,
        "exceptions_raised": raised.iter().map(|e| e.id.to_string()).collect::<Vec<_>>(),
        "exceptions": open_unmatched,
    }))
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
