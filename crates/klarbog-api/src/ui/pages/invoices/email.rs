//! Invoice email POST handler (DK-EMAIL-DELIVERY-001).

use axum::response::Response;
use klarbog_mail::SmtpConfig;
use klarbog_plugin_invoice::{send_invoice_email, EmailKind, InvoiceId};

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::view::load_page;
use crate::AppState;

pub async fn send_email(
    state: &AppState,
    company: &str,
    path: &std::path::Path,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let to_override = form.email_to.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });
    let smtp = SmtpConfig::from_env();
    let dry_run = klarbog_mail::email_dry_run_from_env() || !smtp.is_configured();
    match send_invoice_email(path, &id, EmailKind::Invoice, to_override, &smtp, dry_run).await {
        Ok(outcome) => {
            let msg = if outcome.duplicate {
                format!(
                    "E-mail allerede sendt (idempotent) · message-id {}",
                    outcome.message_id
                )
            } else if dry_run {
                format!(
                    "E-mail dry-run til {} · message-id {}",
                    outcome.recipient, outcome.message_id
                )
            } else {
                format!(
                    "E-mail sendt til {} · message-id {}",
                    outcome.recipient, outcome.message_id
                )
            };
            html_ok(load_page(state, company, msg, String::new()).await)
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}
