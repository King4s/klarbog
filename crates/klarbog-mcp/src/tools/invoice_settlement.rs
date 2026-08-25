//! MCP tools for late compensation and late interest (calc, claim, post preview).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_core::{journal_preview, ConfirmStore};
use klarbog_plugin::Registry;
use klarbog_plugin_invoice::{
    calculate_late_compensation, calculate_late_interest, compensation_post_journal_suggestion,
    get_invoice, interest_post_journal_suggestion, list_invoices,
    oldest_unposted_compensation_claim, parse_iso_date, register_invoice_compensation,
    register_late_interest, resolve_unposted_interest_claim, InvoiceConfig, InvoiceError,
    InvoiceId,
};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

fn map_invoice(err: InvoiceError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn require_confirm(tool: &str, args: &Value) -> Result<(), Envelope<Value>> {
    match args.get("confirm").and_then(|v| v.as_bool()) {
        Some(true) => Ok(()),
        _ => Err(Envelope::err([format!(
            "confirm: true required for {tool}"
        )])),
    }
}

fn resolve_invoice_id(path: &Path, args: &Value) -> Result<InvoiceId, Envelope<Value>> {
    if let Some(id) = args
        .get("invoice_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return Ok(InvoiceId::new(id));
    }
    if let Some(no) = args
        .get("invoice_number")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let invoices = list_invoices(path).map_err(map_invoice)?;
        if let Some(inv) = invoices
            .into_iter()
            .find(|i| i.invoice_no.as_deref() == Some(no))
        {
            return Ok(inv.id);
        }
        return Err(Envelope::err([format!("invoice not found: {no}")]));
    }
    Err(Envelope::err(["missing invoice_id or invoice_number"]))
}

fn parse_as_of(args: &Value) -> Result<chrono::NaiveDate, Envelope<Value>> {
    let raw = args
        .get("as_of")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Envelope::err(["missing as_of"]))?;
    parse_iso_date(raw).map_err(map_invoice)
}

fn parse_compensation_amount_minor(args: &Value) -> Result<Option<i64>, Envelope<Value>> {
    if let Some(minor) = args.get("amount_minor").and_then(|v| v.as_i64()) {
        return Ok(Some(minor));
    }
    let Some(dkk) = args.get("amount_dkk").and_then(|v| v.as_f64()) else {
        return Ok(None);
    };
    if dkk <= 0.0 {
        return Err(Envelope::err(["amount_dkk must be positive"]));
    }
    Ok(Some((dkk * 100.0).round() as i64))
}

fn parse_reference_rate_bps(args: &Value) -> Result<i64, Envelope<Value>> {
    let rate = args
        .get("reference_rate")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| Envelope::err(["missing reference_rate"]))?;
    if rate < 0.0 {
        return Err(Envelope::err(["reference_rate must be non-negative"]));
    }
    Ok((rate * 100.0).round() as i64)
}

async fn authorize_invoice(
    args: &Value,
    allowlist_root: &Path,
) -> Result<(std::path::PathBuf, InvoiceId), Envelope<Value>> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Err(Envelope::err([e])),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Err(Envelope::err([e])),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return Err(map_core_error(e)),
    };
    let invoice_id = resolve_invoice_id(&path, args)?;
    Ok((path, invoice_id))
}

async fn load_invoice(
    path: &Path,
    invoice_id: &InvoiceId,
) -> Result<klarbog_plugin_invoice::Invoice, Envelope<Value>> {
    match get_invoice(path, invoice_id) {
        Ok(Some(inv)) => Ok(inv),
        Ok(None) => Err(Envelope::err([format!("invoice not found: {invoice_id}")])),
        Err(e) => Err(map_invoice(e)),
    }
}

async fn post_preview_response(
    args: &Value,
    allowlist_root: &Path,
    store: &ConfirmStore,
    registry: &Registry,
    invoice: klarbog_plugin_invoice::Invoice,
    journal_entry: klarbog_journal::JournalEntry,
) -> Envelope<Value> {
    let preview = args
        .get("preview")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut data = json!({
        "invoice": invoice,
        "journal_entry": &journal_entry,
    });
    if !preview {
        return Envelope::ok(data);
    }
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let confirm = match journal_preview(
        allowlist_root,
        &company,
        &journal_entry,
        &actor,
        store,
        registry,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    data["confirm_token"] = Value::String(confirm.confirm_token.token.clone());
    data["expires_unix_ms"] = json!(confirm.confirm_token.expires_unix_ms);
    data["payload_digest"] = Value::String(confirm.payload_digest);
    Envelope::ok_with_rules(data, confirm.applied_rules)
}

/// Read-only late-compensation calculation (no register).
pub async fn invoice_compensation_calc(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let (path, invoice_id) = match authorize_invoice(args, allowlist_root).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let as_of = match parse_as_of(args) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let amount_minor = match parse_compensation_amount_minor(args) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let invoice = match load_invoice(&path, &invoice_id).await {
        Ok(inv) => inv,
        Err(e) => return e,
    };
    match calculate_late_compensation(&path, &invoice, as_of, amount_minor) {
        Ok(calc) => Envelope::ok(json!({
            "invoice_id": invoice_id.to_string(),
            "calculation": calc,
        })),
        Err(e) => map_invoice(e),
    }
}

/// Register late-compensation claim (requires confirm:true; no journal post).
pub async fn invoice_claim_compensation(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    if let Err(e) = require_confirm("invoice_claim_compensation", args) {
        return e;
    }
    let (path, invoice_id) = match authorize_invoice(args, allowlist_root).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let as_of = match parse_as_of(args) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let amount_minor = match parse_compensation_amount_minor(args) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    match register_invoice_compensation(&path, &invoice_id, as_of, amount_minor, note) {
        Ok((invoice, calc)) => Envelope::ok(json!({
            "invoice": invoice,
            "calculation": calc,
        })),
        Err(e) => map_invoice(e),
    }
}

/// Journal suggestion for oldest unposted compensation claim; optional ConfirmStore (`preview`).
pub async fn invoice_post_compensation_preview(
    args: &Value,
    allowlist_root: &Path,
    store: &ConfirmStore,
    registry: &Registry,
) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let (path, invoice_id) = match authorize_invoice(args, allowlist_root).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let invoice = match load_invoice(&path, &invoice_id).await {
        Ok(inv) => inv,
        Err(e) => return e,
    };
    let Some((_idx, claim)) = oldest_unposted_compensation_claim(&invoice) else {
        return Envelope::err([format!(
            "{invoice_id} has no unposted compensation claim — call invoice_claim_compensation first"
        )]);
    };
    let entry = match compensation_post_journal_suggestion(
        &invoice,
        claim,
        &actor,
        &InvoiceConfig::default(),
    ) {
        Ok(e) => e,
        Err(e) => return map_invoice(e),
    };
    post_preview_response(args, allowlist_root, store, registry, invoice, entry).await
}

/// Read-only late-interest calculation (no register).
pub async fn invoice_interest_calc(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let (_path, invoice_id) = match authorize_invoice(args, allowlist_root).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let as_of = match parse_as_of(args) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let reference_bps = match parse_reference_rate_bps(args) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let invoice = match load_invoice(&_path, &invoice_id).await {
        Ok(inv) => inv,
        Err(e) => return e,
    };
    match calculate_late_interest(&invoice, as_of, Some(reference_bps)) {
        Ok(calc) => Envelope::ok(json!({
            "invoice_id": invoice_id.to_string(),
            "calculation": calc,
        })),
        Err(e) => map_invoice(e),
    }
}

/// Register late-interest claim (requires confirm:true; no journal post).
pub async fn invoice_claim_interest(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    if let Err(e) = require_confirm("invoice_claim_interest", args) {
        return e;
    }
    let (path, invoice_id) = match authorize_invoice(args, allowlist_root).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let as_of = match parse_as_of(args) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let reference_bps = match parse_reference_rate_bps(args) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    match register_late_interest(&path, &invoice_id, as_of, Some(reference_bps), note) {
        Ok((invoice, calc)) => Envelope::ok(json!({
            "invoice": invoice,
            "calculation": calc,
        })),
        Err(e) => map_invoice(e),
    }
}

/// Journal suggestion for an unposted interest claim; optional ConfirmStore (`preview`).
/// Optional `claim_date` / `reference_rate_bps` select a specific claim; omit → oldest unposted.
pub async fn invoice_post_interest_preview(
    args: &Value,
    allowlist_root: &Path,
    store: &ConfirmStore,
    registry: &Registry,
) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let (path, invoice_id) = match authorize_invoice(args, allowlist_root).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let invoice = match load_invoice(&path, &invoice_id).await {
        Ok(inv) => inv,
        Err(e) => return e,
    };
    let claim_date = args
        .get("claim_date")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let reference_rate_bps = args.get("reference_rate_bps").and_then(|v| v.as_i64());
    let claim = match resolve_unposted_interest_claim(&invoice, claim_date, reference_rate_bps) {
        Ok((_idx, c)) => c,
        Err(e) => return map_invoice(e),
    };
    let entry = match interest_post_journal_suggestion(
        &invoice,
        claim,
        &actor,
        &InvoiceConfig::default(),
    ) {
        Ok(e) => e,
        Err(e) => return map_invoice(e),
    };
    post_preview_response(args, allowlist_root, store, registry, invoice, entry).await
}

#[cfg(test)]
#[path = "invoice_settlement_mcp_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "invoice_settlement_specific_mcp_tests.rs"]
mod specific_claim_tests;
