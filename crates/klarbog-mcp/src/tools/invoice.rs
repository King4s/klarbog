//! Invoice MCP tools — drafts, status, mark-paid previews (no journal write).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_core::{journal_preview, ConfirmStore};
use klarbog_plugin::Registry;
use klarbog_plugin_invoice::{
    create_draft_from_new, get_invoice, journal_suggestion, list_invoices, mark_paid_preview,
    mark_part_paid_preview, patch_status, InvoiceConfig, InvoiceError, InvoiceId, InvoiceKind,
    InvoiceStatus, NewLine,
};
use klarbog_types::{Envelope, PartyId};
use serde_json::{json, Value};
use std::path::Path;

fn map_invoice(err: InvoiceError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn parse_invoice_id(args: &Value) -> Result<InvoiceId, &'static str> {
    match args.get("invoice_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => Ok(InvoiceId::new(id)),
        _ => Err("missing invoice_id"),
    }
}

/// Mirror POST /api/v1/invoices/drafts — create draft + journal suggestion (no post).
pub async fn invoice_create_draft(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let party_id = match args.get("party_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => PartyId::new(id.to_string()),
        _ => return Envelope::err(["missing party_id"]),
    };
    let kind: InvoiceKind = match args.get("kind") {
        Some(v) => match serde_json::from_value(v.clone()) {
            Ok(k) => k,
            Err(_) => return Envelope::err(["invalid kind (expected sale|purchase)"]),
        },
        None => return Envelope::err(["missing kind"]),
    };
    let lines: Vec<NewLine> = match args.get("lines") {
        Some(v) => match serde_json::from_value(v.clone()) {
            Ok(lines) => lines,
            Err(_) => {
                return Envelope::err([
                    "invalid lines (need description, amount_minor i64, currency)",
                ])
            }
        },
        None => return Envelope::err(["missing lines"]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let invoice = match create_draft_from_new(&path, party_id, kind, lines) {
        Ok(inv) => inv,
        Err(e) => return map_invoice(e),
    };
    match journal_suggestion(&invoice, &actor, &InvoiceConfig::default()) {
        Ok(journal_entry) => Envelope::ok(json!({
            "invoice": invoice,
            "journal_entry": journal_entry,
        })),
        Err(e) => map_invoice(e),
    }
}

/// Mirror GET /api/v1/invoices/drafts — list all, or one when `invoice_id` set.
pub async fn invoice_list(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
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
    if let Some(raw_id) = args
        .get("invoice_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let id = InvoiceId::new(raw_id);
        return match get_invoice(&path, &id) {
            Ok(Some(invoice)) => Envelope::ok(serde_json::to_value(invoice).unwrap()),
            Ok(None) => Envelope::err([format!("invoice not found: {id}")]),
            Err(e) => map_invoice(e),
        };
    }
    match list_invoices(&path) {
        Ok(invoices) => Envelope::ok(serde_json::to_value(invoices).unwrap()),
        Err(e) => map_invoice(e),
    }
}

/// Mirror PATCH /api/v1/invoices/status — lifecycle status only (no journal write).
pub async fn invoice_patch_status(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let invoice_id = match parse_invoice_id(args) {
        Ok(id) => id,
        Err(e) => return Envelope::err([e]),
    };
    let status: InvoiceStatus = match args.get("status") {
        Some(v) => match serde_json::from_value(v.clone()) {
            Ok(s) => s,
            Err(_) => {
                return Envelope::err(["invalid status (expected draft|sent|part_paid|paid|void)"])
            }
        },
        None => return Envelope::err(["missing status"]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match patch_status(&path, &invoice_id, status) {
        Ok(invoice) => Envelope::ok(serde_json::to_value(invoice).unwrap()),
        Err(e) => map_invoice(e),
    }
}

/// Mirror POST /api/v1/invoices/mark-paid — suggestion; optional ConfirmStore (`preview`).
pub async fn invoice_mark_paid_preview(
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
    let invoice_id = match parse_invoice_id(args) {
        Ok(id) => id,
        Err(e) => return Envelope::err([e]),
    };
    let preview = args
        .get("preview")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let (invoice, journal_entry) =
        match mark_paid_preview(&path, &invoice_id, &actor, &InvoiceConfig::default()) {
            Ok(v) => v,
            Err(e) => return map_invoice(e),
        };
    let mut data = json!({
        "invoice": invoice,
        "journal_entry": &journal_entry,
    });
    if !preview {
        return Envelope::ok(data);
    }
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

/// Mirror POST /api/v1/invoices/mark-part-paid — suggestion; optional ConfirmStore (`preview`).
pub async fn invoice_mark_part_paid_preview(
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
    let invoice_id = match parse_invoice_id(args) {
        Ok(id) => id,
        Err(e) => return Envelope::err([e]),
    };
    let amount_minor = match args.get("amount_minor").and_then(|v| v.as_i64()) {
        Some(a) => a,
        None => return Envelope::err(["missing amount_minor"]),
    };
    let preview = args
        .get("preview")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let (invoice, journal_entry) = match mark_part_paid_preview(
        &path,
        &invoice_id,
        amount_minor,
        &actor,
        &InvoiceConfig::default(),
    ) {
        Ok(v) => v,
        Err(e) => return map_invoice(e),
    };
    let mut data = json!({
        "invoice": invoice,
        "journal_entry": &journal_entry,
    });
    if !preview {
        return Envelope::ok(data);
    }
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

#[cfg(test)]
#[path = "invoice_preview_tests.rs"]
mod preview_tests;
#[cfg(test)]
#[path = "invoice_tests.rs"]
mod tests;
