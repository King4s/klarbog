//! Journal page render helpers.

use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::super::common::{company_from, foot, html_ok, nav};
use super::form::JournalFields;
use crate::AppState;

#[derive(Template)]
#[template(path = "journal.html")]
pub(super) struct JournalTemplate {
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
    pub memo: String,
    pub account1: String,
    pub direction1: String,
    pub amount1: String,
    pub account2: String,
    pub direction2: String,
    pub amount2: String,
    pub moms_gross: String,
    pub moms_memo: String,
    pub has_moms: bool,
    pub moms_suggested: bool,
    pub moms_reason: String,
    pub moms_gross_minor: String,
    pub moms_net_minor: String,
    pub moms_vat_minor: String,
    pub moms_rate_bps: String,
    pub has_preview: bool,
    pub confirm_token: String,
    pub expires_unix_ms: String,
    pub payload_digest: String,
}

pub(super) fn journal_page(
    state: &AppState,
    company: String,
    fields: JournalFields,
    flash_ok: String,
    flash_err: String,
) -> JournalTemplate {
    let n = nav("journal");
    JournalTemplate {
        title: "Journal",
        nav_home: n.home,
        nav_parties: n.parties,
        nav_invoices: n.invoices,
        nav_bank: n.bank,
        nav_bilag: n.bilag,
        nav_journal: n.journal,
        nav_chart: n.chart,
        nav_settings: n.settings,
        foot: foot(state),
        has_flash_ok: !flash_ok.is_empty(),
        flash_ok,
        has_flash_err: !flash_err.is_empty(),
        flash_err,
        has_company: !company.is_empty(),
        company,
        memo: fields.memo,
        account1: fields.account1,
        direction1: fields.direction1,
        amount1: fields.amount1,
        account2: fields.account2,
        direction2: fields.direction2,
        amount2: fields.amount2,
        moms_gross: fields.moms_gross,
        moms_memo: fields.moms_memo,
        has_moms: fields.has_moms,
        moms_suggested: fields.moms_suggested,
        moms_reason: fields.moms_reason,
        moms_gross_minor: fields.moms_gross_minor,
        moms_net_minor: fields.moms_net_minor,
        moms_vat_minor: fields.moms_vat_minor,
        moms_rate_bps: fields.moms_rate_bps,
        has_preview: fields.has_preview,
        confirm_token: fields.confirm_token,
        expires_unix_ms: fields.expires_unix_ms,
        payload_digest: fields.payload_digest,
    }
}

pub async fn journal_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    html_ok(journal_page(
        &state,
        company,
        JournalFields::default(),
        String::new(),
        String::new(),
    ))
}
