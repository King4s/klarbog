//! Two-phase invoice send (ADR-020): preview books-to-be entry incl. VAT
//! legs; commit posts it and first then flips status to sent.

use std::path::Path;

use axum::response::Response;
use klarbog_core::{journal_commit, load_company_policy};
use klarbog_journal::JournalEntry;
use klarbog_plugin_invoice::{
    get_invoice, journal_suggestion, record_issue, InvoiceConfig, InvoiceId, InvoiceStatus,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::post::preview_with_pending;
use super::view::load_page;
use crate::AppState;

pub(super) async fn send_preview(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let invoice = match get_invoice(path, &id) {
        Ok(Some(inv)) => inv,
        Ok(None) => {
            return html_ok(
                load_page(state, company, String::new(), format!("Ukendt: {id}")).await,
            );
        }
        Err(e) => {
            return html_ok(load_page(state, company, String::new(), e.to_string()).await);
        }
    };
    if invoice.status != InvoiceStatus::Draft {
        return html_ok(
            load_page(
                state,
                company,
                String::new(),
                format!("{id} er ikke en kladde (status {:?})", invoice.status),
            )
            .await,
        );
    }
    match journal_suggestion(&invoice, actor, &InvoiceConfig::default()) {
        Ok(entry) => {
            preview_with_pending(state, company, actor, &id, entry, "Send", "commit_send").await
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn commit_send(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.unwrap_or_default());
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
    let issue_date = entry.as_of.format("%Y-%m-%d").to_string();
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
        Ok(r) => {
            let payment_terms = load_company_policy(path)
                .map(|p| p.payment_terms_days())
                .unwrap_or(30);
            match record_issue(path, &id, issue_date, payment_terms) {
                Ok(_) => html_ok(
                    load_page(
                        state,
                        company,
                        format!(
                            "Faktura bogført · posted {} · {id} sat til sent",
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
                        format!("Bogført ({}) men statusskift fejlede: {e}", r.posted.id),
                    )
                    .await,
                ),
            }
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}
