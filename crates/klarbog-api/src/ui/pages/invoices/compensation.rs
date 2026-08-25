//! Fast kompensation: registrering + to-faset bogføring (DK-INVOICE-LATE-COMPENSATION-001).

use std::path::Path;

use axum::response::Response;
use chrono::Utc;
use klarbog_core::journal_commit;
use klarbog_journal::JournalEntry;
use klarbog_plugin_crm::{get_party, PartyKind};
use klarbog_plugin_invoice::{
    compensation_post_journal_suggestion, get_invoice, mark_compensation_posted,
    oldest_unposted_compensation_claim, parse_iso_date, register_invoice_compensation, Invoice,
    InvoiceConfig, InvoiceId,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::post::preview_with_pending;
use super::view::load_page;
use crate::AppState;

fn compensable(path: &Path, id: &InvoiceId) -> Result<Invoice, String> {
    let invoice = match get_invoice(path, id) {
        Ok(Some(inv)) => inv,
        Ok(None) => return Err(format!("Ukendt: {id}")),
        Err(e) => return Err(e.to_string()),
    };
    if !invoice.status.allows_late_interest() {
        return Err(format!(
            "{id} kan ikke fast kompensation (status {:?})",
            invoice.status
        ));
    }
    let party = get_party(path, &invoice.party_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Part {} findes ikke", invoice.party_id))?;
    if party.kind != PartyKind::Business {
        return Err(format!("{id} kræver erhvervskunde (business part)"));
    }
    Ok(invoice)
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

fn parse_compensation_minor(raw: &Option<String>) -> Result<Option<i64>, String> {
    let Some(s) = raw.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    s.parse::<i64>()
        .map(Some)
        .map_err(|_| format!("Kompensationsbeløb skal være heltal i øre, fik {s}"))
}

pub(super) async fn compensation_register(
    state: &AppState,
    company: &str,
    path: &Path,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    if compensable(path, &id).is_err() {
        return html_ok(
            load_page(
                state,
                company,
                String::new(),
                compensable(path, &id).unwrap_err(),
            )
            .await,
        );
    }
    let as_of = match parse_as_of(&form.as_of_date) {
        Ok(d) => d,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let amount_minor = match parse_compensation_minor(&form.compensation_amount_minor) {
        Ok(a) => a,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    match register_invoice_compensation(
        path,
        &id,
        as_of,
        amount_minor,
        form.compensation_note.clone(),
    ) {
        Ok((_inv, calc)) => html_ok(
            load_page(
                state,
                company,
                format!(
                    "Fast kompensation registreret · {id} · {} øre ({} d forfalden)",
                    calc.compensation_amount_minor, calc.overdue_days
                ),
                String::new(),
            )
            .await,
        ),
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn compensation_post_preview(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let invoice = match compensable(path, &id) {
        Ok(inv) => inv,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let Some((_idx, claim)) = oldest_unposted_compensation_claim(&invoice) else {
        return html_ok(
            load_page(
                state,
                company,
                String::new(),
                format!("{id} har ingen ubogført kompensation — registrer først"),
            )
            .await,
        );
    };
    let cfg = InvoiceConfig::default();
    match compensation_post_journal_suggestion(&invoice, claim, actor, &cfg) {
        Ok(entry) => {
            preview_with_pending(
                state,
                company,
                actor,
                &id,
                entry,
                "Fast kompensation",
                "commit_compensation",
            )
            .await
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn commit_compensation(
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
    let claim_date = match extract_compensation_claim_date(&entry.memo) {
        Some(d) => d,
        None => {
            return html_ok(
                load_page(
                    state,
                    company,
                    String::new(),
                    "Kompensations-memo mangler claim-dato — kør preview igen".into(),
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
        Ok(r) => match mark_compensation_posted(path, &id, &claim_date, &r.posted.id.to_string()) {
            Ok(_) => html_ok(
                load_page(
                    state,
                    company,
                    format!(
                        "Fast kompensation bogført · posted {} · {id} · claim {claim_date}",
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
                        "Kompensation bogført ({}) men claim-link fejlede: {e}",
                        r.posted.id
                    ),
                )
                .await,
            ),
        },
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

fn extract_compensation_claim_date(memo: &str) -> Option<String> {
    let tail = memo.split(":compensation:").nth(1)?;
    let date = tail.split('·').next()?.trim();
    if parse_iso_date(date).is_ok() {
        Some(date.to_string())
    } else {
        None
    }
}
