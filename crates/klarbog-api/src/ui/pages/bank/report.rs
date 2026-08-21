//! Periode-afstemningsrapport (DK-BOOKKEEPING-RECONCILIATION-001, §11):
//! parse det indsatte CSV, hent posterede `bank:`-referencer og vis
//! matchede/umatchede transaktioner med beløbssummer for perioden.

use axum::response::Response;
use chrono::{DateTime, NaiveDate};
use klarbog_plugin_bank::{
    default_source_for_rail, import_preview, reconciliation_report, BankImportConfig,
    BankPostedRef, BankProfile,
};
use klarbog_types::Actor;

use super::super::common::{format_dkk, html_ok};
use super::form::{BankActionForm, ReconReportRow};
use super::view::{bank_page, BankView};
use crate::AppState;

fn parse_date(raw: &str, label: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|e| format!("Ugyldig {label} (YYYY-MM-DD): {e}"))
}

fn posted_refs(raw: Vec<klarbog_core::BankMemoRef>) -> Vec<BankPostedRef> {
    raw.into_iter()
        .filter_map(|r| {
            let date = DateTime::parse_from_rfc3339(&r.as_of)
                .map(|d| d.date_naive())
                .ok()?;
            Some(BankPostedRef {
                as_of_date: date,
                memo: r.memo,
            })
        })
        .collect()
}

fn to_row(r: &klarbog_plugin_bank::ReconciliationRow) -> ReconReportRow {
    ReconReportRow {
        date: r.date.format("%Y-%m-%d").to_string(),
        text: r.text.clone(),
        dkk: format_dkk(r.amount_minor),
        memo: r.matched_memo.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn report(
    state: &AppState,
    company: String,
    path: &std::path::Path,
    actor: &Actor,
    provider: BankProfile,
    provider_s: String,
    cfg: &BankImportConfig,
    form: &BankActionForm,
) -> Response {
    let err_page = |msg: String| {
        bank_page(
            state,
            BankView {
                company: company.clone(),
                provider: provider_s.clone(),
                csv: form.csv.clone(),
                period_from: form.period_from.clone(),
                period_to: form.period_to.clone(),
                flash_err: msg,
                ..Default::default()
            },
        )
    };
    if form.csv.trim().is_empty() {
        return html_ok(err_page(
            "CSV kræves til rapport — kør import-preview først.".into(),
        ));
    }
    let from = match parse_date(&form.period_from, "fra-dato") {
        Ok(d) => d,
        Err(e) => return html_ok(err_page(e)),
    };
    let to = match parse_date(&form.period_to, "til-dato") {
        Ok(d) => d,
        Err(e) => return html_ok(err_page(e)),
    };
    let source = default_source_for_rail(provider);
    let rows = match import_preview(
        source,
        provider,
        Some(form.csv.trim()),
        cfg,
        actor,
        Some(path),
    )
    .await
    {
        Ok((rows, _)) => rows,
        Err(e) => return html_ok(err_page(e.to_string())),
    };
    let store = match klarbog_core::open_existing(path).await {
        Ok(c) => c,
        Err(e) => return html_ok(err_page(e.to_string())),
    };
    let posted = match store.bank_posted_refs().await {
        Ok(p) => posted_refs(p),
        Err(e) => return html_ok(err_page(e.to_string())),
    };
    match reconciliation_report(&rows, &posted, from, to) {
        Ok(r) => html_ok(bank_page(
            state,
            BankView {
                company,
                provider: provider_s,
                csv: form.csv.clone(),
                period_from: form.period_from.clone(),
                period_to: form.period_to.clone(),
                recon_matched: r.matched.iter().map(to_row).collect(),
                recon_unmatched: r.unmatched.iter().map(to_row).collect(),
                recon_ran: true,
                recon_matched_total: format_dkk(r.matched_amount_minor),
                recon_unmatched_total: format_dkk(r.unmatched_amount_minor),
                flash_ok: format!(
                    "Afstemningsrapport {from} – {to} · {} matchede · {} umatchede",
                    r.matched.len(),
                    r.unmatched.len()
                ),
                ..Default::default()
            },
        )),
        Err(e) => html_ok(err_page(e.to_string())),
    }
}
