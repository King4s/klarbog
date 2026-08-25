//! Morarente: registrering + to-faset bogføring (DK-INVOICE-LATE-INTEREST-001).

use std::path::Path;

use axum::response::Response;
use chrono::Utc;
use klarbog_core::journal_commit;
use klarbog_journal::JournalEntry;
use klarbog_plugin_invoice::{
    get_invoice, interest_post_journal_suggestion, mark_interest_claim_posted, parse_iso_date,
    register_late_interest, resolve_unposted_interest_claim, Invoice, InvoiceConfig, InvoiceId,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::post::preview_with_pending;
use super::view::load_page;
use crate::AppState;

fn interestable(path: &Path, id: &InvoiceId) -> Result<Invoice, String> {
    let invoice = match get_invoice(path, id) {
        Ok(Some(inv)) => inv,
        Ok(None) => return Err(format!("Ukendt: {id}")),
        Err(e) => return Err(e.to_string()),
    };
    if !invoice.status.allows_late_interest() {
        return Err(format!(
            "{id} kan ikke morarente (status {:?})",
            invoice.status
        ));
    }
    Ok(invoice)
}

fn parse_reference_bps(raw: &Option<String>) -> Result<Option<i64>, String> {
    let Some(s) = raw.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    s.parse::<i64>()
        .map(Some)
        .map_err(|_| format!("Referencesats skal være heltal (bps, fx 220 for 2,2 %), fik {s}"))
}

fn parse_as_of(raw: &Option<String>) -> Result<chrono::NaiveDate, String> {
    let s = raw
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Utc::now().date_naive().format("%Y-%m-%d").to_string());
    parse_iso_date(&s).map_err(|e| e.to_string())
}

pub(super) async fn interest_register(
    state: &AppState,
    company: &str,
    path: &Path,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    if interestable(path, &id).is_err() {
        return html_ok(
            load_page(
                state,
                company,
                String::new(),
                interestable(path, &id).unwrap_err(),
            )
            .await,
        );
    }
    let as_of = match parse_as_of(&form.as_of_date) {
        Ok(d) => d,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let reference_bps = match parse_reference_bps(&form.reference_rate_bps) {
        Ok(r) => r,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    match register_late_interest(path, &id, as_of, reference_bps, form.interest_note.clone()) {
        Ok((_inv, calc)) => html_ok(
            load_page(
                state,
                company,
                format!(
                    "Morarente registreret · {id} · {} øre ({} d, mora {} bps)",
                    calc.accrued_interest_minor, calc.claimable_days, calc.annual_interest_rate_bps
                ),
                String::new(),
            )
            .await,
        ),
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn interest_post_preview(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let invoice = match interestable(path, &id) {
        Ok(inv) => inv,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let claim_date_key = form
        .as_of_date
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let rate_key = match parse_reference_bps(&form.reference_rate_bps) {
        Ok(r) => r,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let claim = match resolve_unposted_interest_claim(&invoice, claim_date_key, rate_key) {
        Ok((_idx, c)) => c,
        Err(e) => {
            return html_ok(load_page(state, company, String::new(), format!("{id}: {e}")).await);
        }
    };
    let cfg = InvoiceConfig::default();
    match interest_post_journal_suggestion(&invoice, claim, actor, &cfg) {
        Ok(entry) => {
            preview_with_pending(
                state,
                company,
                actor,
                &id,
                entry,
                "Morarente",
                "commit_interest",
            )
            .await
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn commit_interest(
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
    let (claim_date, claim_rate) = match extract_interest_claim_key(&entry.memo) {
        Some(k) => k,
        None => {
            return html_ok(
                load_page(
                    state,
                    company,
                    String::new(),
                    "Morarente-memo mangler claim-dato — kør preview igen".into(),
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
        Ok(r) => match mark_interest_claim_posted(
            path,
            &id,
            &claim_date,
            claim_rate,
            &r.posted.id.to_string(),
        ) {
            Ok(_) => html_ok(
                load_page(
                    state,
                    company,
                    format!(
                        "Morarente bogført · posted {} · {id} · claim {claim_date}",
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
                        "Morarente bogført ({}) men claim-link fejlede: {e}",
                        r.posted.id
                    ),
                )
                .await,
            ),
        },
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

/// Memo key: `…:interest:YYYY-MM-DD@rate · …` (rate optional for older memos).
fn extract_interest_claim_key(memo: &str) -> Option<(String, Option<i64>)> {
    let tail = memo.split(":interest:").nth(1)?;
    let key = tail.split('·').next()?.trim();
    let (date, rate) = match key.split_once('@') {
        Some((d, r)) => (d.trim(), r.trim().parse::<i64>().ok()),
        None => (key, None),
    };
    if parse_iso_date(date).is_ok() {
        Some((date.to_string(), rate))
    } else {
        None
    }
}
