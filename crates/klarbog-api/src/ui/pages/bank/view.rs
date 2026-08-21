//! Bank page render + GET.

use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::super::common::{company_from, foot, html_ok, nav};
use super::form::{DraftRow, ReconReportRow, SuggestRow};
use crate::AppState;

#[derive(Template)]
#[template(path = "bank.html")]
pub(super) struct BankTemplate {
    pub title: &'static str,
    pub nav_home: bool,
    pub nav_parties: bool,
    pub nav_invoices: bool,
    pub nav_bank: bool,
    pub nav_bilag: bool,
    pub nav_journal: bool,
    pub nav_chart: bool,
    pub nav_settings: bool,
    pub foot: String,
    pub has_flash_ok: bool,
    pub flash_ok: String,
    pub has_flash_err: bool,
    pub flash_err: String,
    pub has_company: bool,
    pub company: String,
    pub provider: String,
    pub csv: String,
    pub row_date: String,
    pub row_text: String,
    pub row_amount: String,
    pub has_import: bool,
    pub import_count: String,
    pub import_source: String,
    pub drafts: Vec<DraftRow>,
    pub has_suggest: bool,
    pub suggest_count: String,
    pub suggestions: Vec<SuggestRow>,
    pub has_pending: bool,
    pub pending_label: String,
    pub pending_entry_json: String,
    pub pending_token: String,
    pub pending_invoice_id: String,
    pub period_from: String,
    pub period_to: String,
    pub has_recon: bool,
    pub recon_matched_count: String,
    pub recon_unmatched_count: String,
    pub recon_matched_total: String,
    pub recon_unmatched_total: String,
    pub recon_matched: Vec<ReconReportRow>,
    pub recon_unmatched: Vec<ReconReportRow>,
}

#[derive(Default)]
pub(super) struct BankView {
    pub company: String,
    pub provider: String,
    pub csv: String,
    pub row_date: String,
    pub row_text: String,
    pub row_amount: String,
    pub drafts: Vec<DraftRow>,
    pub import_source: String,
    pub suggestions: Vec<SuggestRow>,
    pub flash_ok: String,
    pub flash_err: String,
    pub pending_label: String,
    pub pending_entry_json: String,
    pub pending_token: String,
    pub pending_invoice_id: String,
    pub period_from: String,
    pub period_to: String,
    pub recon_matched: Vec<ReconReportRow>,
    pub recon_unmatched: Vec<ReconReportRow>,
    pub recon_ran: bool,
    pub recon_matched_total: String,
    pub recon_unmatched_total: String,
}

pub(super) fn bank_page(state: &AppState, v: BankView) -> BankTemplate {
    let n = nav("bank");
    BankTemplate {
        title: "Bank",
        nav_home: n.home,
        nav_parties: n.parties,
        nav_invoices: n.invoices,
        nav_bank: n.bank,
        nav_bilag: n.bilag,
        nav_journal: n.journal,
        nav_chart: n.chart,
        nav_settings: n.settings,
        foot: foot(state),
        has_flash_ok: !v.flash_ok.is_empty(),
        flash_ok: v.flash_ok,
        has_flash_err: !v.flash_err.is_empty(),
        flash_err: v.flash_err,
        has_company: !v.company.is_empty(),
        company: v.company,
        provider: v.provider,
        csv: v.csv,
        row_date: v.row_date,
        row_text: v.row_text,
        row_amount: v.row_amount,
        has_import: !v.drafts.is_empty(),
        import_count: v.drafts.len().to_string(),
        import_source: v.import_source,
        drafts: v.drafts,
        has_suggest: !v.suggestions.is_empty(),
        suggest_count: v.suggestions.len().to_string(),
        suggestions: v.suggestions,
        has_pending: !v.pending_token.is_empty(),
        pending_label: v.pending_label,
        pending_entry_json: v.pending_entry_json,
        pending_token: v.pending_token,
        pending_invoice_id: v.pending_invoice_id,
        period_from: v.period_from,
        period_to: v.period_to,
        has_recon: v.recon_ran,
        recon_matched_count: v.recon_matched.len().to_string(),
        recon_unmatched_count: v.recon_unmatched.len().to_string(),
        recon_matched_total: v.recon_matched_total,
        recon_unmatched_total: v.recon_unmatched_total,
        recon_matched: v.recon_matched,
        recon_unmatched: v.recon_unmatched,
    }
}

pub async fn bank_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    html_ok(bank_page(
        &state,
        BankView {
            company,
            provider: "generic_dk".into(),
            row_date: "2026-05-20".into(),
            row_text: "Customer payment".into(),
            row_amount: "50000".into(),
            ..Default::default()
        },
    ))
}
