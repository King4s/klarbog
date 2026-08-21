//! Two-phase credit note (ADR-020): preview exact-negates the send booking
//! (incl. VAT leg); commit posts it and first then flips status to void.

use std::path::Path;

use axum::response::Response;
use klarbog_core::journal_commit;
use klarbog_journal::JournalEntry;
use klarbog_plugin_invoice::{
    credit_journal_suggestion, get_invoice, patch_status, Invoice, InvoiceConfig, InvoiceId,
    InvoiceStatus,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::post::preview_with_pending;
use super::view::load_page;
use crate::AppState;

/// Fail-closed re-check used at both preview and commit: the invoice must
/// still be sent and unpaid when the credit note is booked.
fn creditable(path: &Path, id: &InvoiceId) -> Result<Invoice, String> {
    let invoice = match get_invoice(path, id) {
        Ok(Some(inv)) => inv,
        Ok(None) => return Err(format!("Ukendt: {id}")),
        Err(e) => return Err(e.to_string()),
    };
    if invoice.status != InvoiceStatus::Sent {
        return Err(format!(
            "{id} kan ikke krediteres (status {:?}, kræver sent)",
            invoice.status
        ));
    }
    if !invoice.payments.is_empty() {
        return Err(format!(
            "{id} har registrerede betalinger — fuld kreditnota afvist"
        ));
    }
    Ok(invoice)
}

pub(super) async fn credit_preview(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let reason = form.credit_reason.clone().unwrap_or_default();
    let invoice = match creditable(path, &id) {
        Ok(inv) => inv,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    match credit_journal_suggestion(&invoice, &reason, actor, &InvoiceConfig::default()) {
        Ok(entry) => {
            preview_with_pending(
                state,
                company,
                actor,
                &id,
                entry,
                "Kreditnota",
                "commit_credit",
            )
            .await
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn commit_credit(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.unwrap_or_default());
    if let Err(e) = creditable(path, &id) {
        return html_ok(load_page(state, company, String::new(), e).await);
    }
    let token = form.confirm_token.unwrap_or_default();
    let entry: JournalEntry = match serde_json::from_str(form.entry_json.unwrap_or_default().trim())
    {
        Ok(e) => e,
        Err(e) => {
            return html_ok(
                load_page(
                    state,
                    company,
                    String::new(),
                    format!("Ugyldig entry_json: {e}"),
                )
                .await,
            );
        }
    };
    match journal_commit(
        &state.allowlist_root,
        Path::new(company),
        entry,
        actor,
        token.trim(),
        &state.confirm,
        &state.registry,
    )
    .await
    {
        Ok(r) => match patch_status(path, &id, InvoiceStatus::Void) {
            Ok(_) => html_ok(
                load_page(
                    state,
                    company,
                    format!(
                        "Kreditnota bogført · posted {} · {id} sat til void",
                        r.posted.id
                    ),
                    String::new(),
                )
                .await,
            ),
            Err(e) => html_ok(
                load_page(
                    state,
                    company,
                    String::new(),
                    format!(
                        "Kreditnota bogført ({}) men statusskift fejlede: {e}",
                        r.posted.id
                    ),
                )
                .await,
            ),
        },
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}
