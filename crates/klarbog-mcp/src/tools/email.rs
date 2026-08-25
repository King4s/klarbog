//! MCP email delivery (DK-EMAIL-DELIVERY-001) — mirrors UI send_email / send_reminder mail step.

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_mail::SmtpConfig;
use klarbog_plugin_invoice::{
    get_invoice, list_invoices, send_invoice_email, EmailKind, InvoiceError, InvoiceId,
};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

fn map_invoice(err: InvoiceError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn require_confirm(args: &Value) -> Result<(), Envelope<Value>> {
    match args.get("confirm").and_then(|v| v.as_bool()) {
        Some(true) => Ok(()),
        _ => Err(Envelope::err([
            "confirm: true required for invoice_send_email",
        ])),
    }
}

fn parse_email_kind(args: &Value) -> EmailKind {
    match args.get("kind").and_then(|v| v.as_str()) {
        Some("reminder") => EmailKind::Reminder,
        _ => EmailKind::Invoice,
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

/// Send issued invoice or reminder email (append-only log; idempotent).
pub async fn invoice_send_email(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    if let Err(e) = require_confirm(args) {
        return e;
    }
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
    let invoice_id = match resolve_invoice_id(&path, args) {
        Ok(id) => id,
        Err(e) => return e,
    };
    match get_invoice(&path, &invoice_id) {
        Ok(Some(_)) => {}
        Ok(None) => return Envelope::err([format!("invoice not found: {invoice_id}")]),
        Err(e) => return map_invoice(e),
    }
    let kind = parse_email_kind(args);
    let to_override = args
        .get("to")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let smtp = SmtpConfig::from_env();
    let dry_run = klarbog_mail::email_dry_run_from_env() || !smtp.is_configured();
    match send_invoice_email(&path, &invoice_id, kind, to_override, &smtp, dry_run).await {
        Ok(outcome) => {
            let invoice = match get_invoice(&path, &invoice_id) {
                Ok(Some(inv)) => inv,
                Ok(None) => return Envelope::err([format!("invoice not found: {invoice_id}")]),
                Err(e) => return map_invoice(e),
            };
            Envelope::ok(json!({
                "invoice_id": invoice_id.to_string(),
                "invoice_number": invoice.invoice_no,
                "kind": kind,
                "recipient": outcome.recipient,
                "subject": outcome.subject,
                "message_id": outcome.message_id,
                "duplicate": outcome.duplicate,
                "dry_run": dry_run,
            }))
        }
        Err(e) => map_invoice(e),
    }
}

#[cfg(test)]
#[path = "email_tests.rs"]
mod tests;
