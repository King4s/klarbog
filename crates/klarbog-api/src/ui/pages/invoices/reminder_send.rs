//! Compound send-reminder (register + inline book + email), mirroring TS cockpit.

use std::path::Path;

use axum::response::Response;
use klarbog_core::{journal_commit, journal_preview};
use klarbog_mail::SmtpConfig;
use klarbog_plugin_invoice::{
    get_invoice, looks_like_email, mark_reminder_posted, register_invoice_reminder,
    reminder_post_journal_suggestion, rollback_unposted_reminder, send_invoice_email, EmailKind,
    Invoice, InvoiceConfig, InvoiceId,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::reminders::{parse_fee_minor, parse_reminder_date, remindable};
use super::view::load_page;
use crate::AppState;

fn skip_reminder_book(form: &InvoiceActionForm) -> bool {
    form.reminder_skip_book
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
}

fn resolve_recipient(
    path: &Path,
    invoice: &Invoice,
    to_override: Option<&str>,
) -> Result<String, String> {
    let recipient = if let Some(explicit) = to_override.filter(|s| !s.trim().is_empty()) {
        explicit.trim().to_string()
    } else {
        let party = klarbog_plugin_crm::get_party(path, &invoice.party_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Ukendt part {}", invoice.party_id))?;
        party
            .email
            .filter(|e| !e.trim().is_empty())
            .ok_or_else(|| {
                "Modtager mangler e-mail — angiv email_to eller tilføj e-mail på parten".to_string()
            })?
    };
    if !looks_like_email(&recipient) {
        return Err(format!(
            "Modtagerens e-mailadresse er ugyldig ({recipient}) — rykkeren kan ikke sendes"
        ));
    }
    Ok(recipient)
}

fn prevalidate_reminder_send(
    path: &Path,
    id: &InvoiceId,
    to_override: Option<&str>,
    smtp: &SmtpConfig,
    dry_run: bool,
) -> Result<(Invoice, String), String> {
    let invoice = remindable(path, id)?;
    let recipient = resolve_recipient(path, &invoice, to_override)?;
    let invoice_no = invoice
        .invoice_no
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{id} mangler udstedt fakturanummer"))?;
    let issued_path = path.join(format!("objects/invoices/issued/{invoice_no}.json"));
    if !issued_path.is_file() {
        return Err(format!(
            "Udstedt faktura-JSON mangler ({}) — rykker-mail kan ikke sendes",
            issued_path.display()
        ));
    }
    if !dry_run && !smtp.is_configured() {
        return Err(
            "SMTP er ikke konfigureret — sæt SMTP_SYSTEM_* eller KLARBOG_EMAIL_DRY_RUN=1".into(),
        );
    }
    if !looks_like_email(&smtp.from_email) {
        return Err(format!(
            "Afsender-e-mail er ugyldig ({}) — rykkeren kan ikke sendes",
            smtp.from_email
        ));
    }
    Ok((invoice, recipient))
}

async fn book_reminder_fee_inline(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    id: &InvoiceId,
    reminder_date: &str,
) -> Result<String, String> {
    let invoice = get_invoice(path, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Ukendt: {id}"))?;
    let reminder = invoice
        .reminders
        .iter()
        .find(|r| r.reminder_date == reminder_date && r.posted_journal_id.is_none())
        .ok_or_else(|| format!("{id} har ingen ubogført rykker {reminder_date}"))?;
    let entry =
        reminder_post_journal_suggestion(&invoice, reminder, actor, &InvoiceConfig::default())
            .map_err(|e| e.to_string())?;
    let preview = journal_preview(
        &state.allowlist_root,
        Path::new(company),
        &entry,
        actor,
        &state.confirm,
        &state.registry,
    )
    .await
    .map_err(|e| e.to_string())?;
    let posted = journal_commit(
        &state.allowlist_root,
        Path::new(company),
        entry,
        actor,
        preview.confirm_token.token.trim(),
        &state.confirm,
        &state.registry,
    )
    .await
    .map_err(|e| e.to_string())?;
    mark_reminder_posted(path, id, reminder_date, &posted.posted.id.to_string())
        .map_err(|e| e.to_string())?;
    Ok(posted.posted.id.to_string())
}

pub(super) async fn send_reminder(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let to_override = form.email_to.as_deref();
    let smtp = SmtpConfig::from_env();
    let dry_run = klarbog_mail::email_dry_run_from_env() || !smtp.is_configured();
    let (invoice, recipient) =
        match prevalidate_reminder_send(path, &id, to_override, &smtp, dry_run) {
            Ok(v) => v,
            Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
        };
    let reminder_date = match parse_reminder_date(&form.reminder_date) {
        Ok(d) => d,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let fee_minor = match parse_fee_minor(&form.reminder_fee_minor) {
        Ok(f) => f,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let reminder_date_s = reminder_date.format("%Y-%m-%d").to_string();
    let book_fee = !skip_reminder_book(form);

    let registered = match register_invoice_reminder(
        path,
        &id,
        reminder_date,
        fee_minor,
        form.reminder_note.clone(),
    ) {
        Ok((_inv, result)) => result,
        Err(e) => return html_ok(load_page(state, company, String::new(), e.to_string()).await),
    };

    let mut journal_id: Option<String> = None;
    if book_fee {
        match book_reminder_fee_inline(state, company, path, actor, &id, &reminder_date_s).await {
            Ok(jid) => journal_id = Some(jid),
            Err(e) => {
                let _ = rollback_unposted_reminder(path, &id, &reminder_date_s);
                return html_ok(load_page(state, company, String::new(), e).await);
            }
        }
    }

    match send_invoice_email(
        path,
        &id,
        EmailKind::Reminder,
        Some(&recipient),
        &smtp,
        dry_run,
    )
    .await
    {
        Ok(outcome) => {
            let mail_note = if outcome.duplicate {
                format!("rykkermail idempotent ({})", outcome.message_id)
            } else if dry_run {
                format!("rykkermail dry-run til {}", outcome.recipient)
            } else {
                format!("rykkermail sendt til {}", outcome.recipient)
            };
            let book_note = if let Some(jid) = journal_id {
                format!(" · gebyr bogført {jid}")
            } else {
                String::new()
            };
            html_ok(
                load_page(
                    state,
                    company,
                    format!(
                        "Rykker sendt · {id} · nr {} · {} · {}{book_note}",
                        registered.reminder_sequence,
                        invoice.invoice_no.as_deref().unwrap_or("?"),
                        mail_note
                    ),
                    String::new(),
                )
                .await,
            )
        }
        Err(e) => {
            if journal_id.is_none() {
                let _ = rollback_unposted_reminder(path, &id, &reminder_date_s);
            }
            html_ok(load_page(state, company, String::new(), e.to_string()).await)
        }
    }
}
