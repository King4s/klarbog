//! Rykkergebyr: registrering + to-faset bogføring (DK-INVOICE-REMINDER-FEE-001).

use std::path::Path;

use axum::response::Response;
use chrono::Utc;
use klarbog_core::journal_commit;
use klarbog_journal::JournalEntry;
use klarbog_plugin_invoice::{
    get_invoice, mark_reminder_posted, parse_iso_date, register_invoice_reminder,
    reminder_post_journal_suggestion, resolve_unposted_reminder, Invoice, InvoiceConfig, InvoiceId,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::post::preview_with_pending;
use super::view::load_page;
use crate::AppState;

pub(super) fn remindable(path: &Path, id: &InvoiceId) -> Result<Invoice, String> {
    let invoice = match get_invoice(path, id) {
        Ok(Some(inv)) => inv,
        Ok(None) => return Err(format!("Ukendt: {id}")),
        Err(e) => return Err(e.to_string()),
    };
    if !invoice.status.allows_reminder() {
        return Err(format!(
            "{id} kan ikke rykker (status {:?})",
            invoice.status
        ));
    }
    Ok(invoice)
}

pub(super) fn parse_reminder_date(raw: &Option<String>) -> Result<chrono::NaiveDate, String> {
    let s = raw
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Utc::now().date_naive().format("%Y-%m-%d").to_string());
    parse_iso_date(&s).map_err(|e| e.to_string())
}

pub(super) fn parse_fee_minor(raw: &Option<String>) -> Result<Option<i64>, String> {
    let Some(s) = raw.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    s.parse::<i64>()
        .map(Some)
        .map_err(|_| format!("Rykkergebyr skal være heltal i øre, fik {s}"))
}

pub(super) async fn reminder_register(
    state: &AppState,
    company: &str,
    path: &Path,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    if remindable(path, &id).is_err() {
        return html_ok(
            load_page(
                state,
                company,
                String::new(),
                remindable(path, &id).unwrap_err(),
            )
            .await,
        );
    }
    let reminder_date = match parse_reminder_date(&form.reminder_date) {
        Ok(d) => d,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let fee_minor = match parse_fee_minor(&form.reminder_fee_minor) {
        Ok(f) => f,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    match register_invoice_reminder(
        path,
        &id,
        reminder_date,
        fee_minor,
        form.reminder_note.clone(),
    ) {
        Ok((_inv, result)) => html_ok(
            load_page(
                state,
                company,
                format!(
                    "Rykker registreret · {id} · nr {} · {} øre (total {} øre)",
                    result.reminder_sequence,
                    result.fee_amount_minor,
                    result.total_reminder_fees_minor
                ),
                String::new(),
            )
            .await,
        ),
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn reminder_post_preview(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let invoice = match remindable(path, &id) {
        Ok(inv) => inv,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let reminder_date_key = form
        .reminder_date
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let reminder = match resolve_unposted_reminder(&invoice, reminder_date_key, None) {
        Ok((_idx, r)) => r,
        Err(e) => {
            return html_ok(load_page(state, company, String::new(), format!("{id}: {e}")).await);
        }
    };
    let cfg = InvoiceConfig::default();
    match reminder_post_journal_suggestion(&invoice, reminder, actor, &cfg) {
        Ok(entry) => {
            preview_with_pending(
                state,
                company,
                actor,
                &id,
                entry,
                "Rykkergebyr",
                "commit_reminder",
            )
            .await
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn commit_reminder(
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
    let reminder_date = match extract_reminder_date(&entry.memo) {
        Some(d) => d,
        None => {
            return html_ok(
                load_page(
                    state,
                    company,
                    String::new(),
                    "Rykker-memo mangler dato — kør preview igen".into(),
                )
                .await,
            );
        }
    };
    match journal_commit(
        &state.allowlist_root,
        Path::new(company),
        entry.clone(),
        actor,
        token.trim(),
        &state.confirm,
        &state.registry,
    )
    .await
    {
        Ok(r) => match mark_reminder_posted(path, &id, &reminder_date, &r.posted.id.to_string()) {
            Ok(_) => html_ok(
                load_page(
                    state,
                    company,
                    format!(
                        "Rykkergebyr bogført · posted {} · {id} · rykker {reminder_date}",
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
                        "Rykkergebyr bogført ({}) men reminder-link fejlede: {e}",
                        r.posted.id
                    ),
                )
                .await,
            ),
        },
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

fn extract_reminder_date(memo: &str) -> Option<String> {
    let tail = memo.split(":reminder:").nth(1)?;
    let date = tail.split('·').next()?.trim();
    if parse_iso_date(date).is_ok() {
        Some(date.to_string())
    } else {
        None
    }
}
