//! Bilag page render + GET.

use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_plugin_documents::{list_documents, list_exceptions, ExceptionSeverity};
use klarbog_plugin_retention::load_retention;

use super::super::common::{authorize_company, company_from, foot, html_ok, nav};
use crate::AppState;

pub(super) struct DocRow {
    id: String,
    kind: String,
    path_hint: String,
    notes: String,
}

pub(super) struct ExcRow {
    id: String,
    code: String,
    severity: String,
    message: String,
    status: String,
}

#[derive(Template)]
#[template(path = "bilag.html")]
pub(super) struct BilagTemplate {
    title: &'static str,
    nav_home: bool,
    nav_parties: bool,
    nav_invoices: bool,
    nav_bank: bool,
    nav_bilag: bool,
    nav_journal: bool,
    nav_chart: bool,
    nav_settings: bool,
    foot: String,
    has_flash_ok: bool,
    flash_ok: String,
    has_flash_err: bool,
    flash_err: String,
    has_company: bool,
    company: String,
    retain_days: String,
    purge_after_days: String,
    documents: Vec<DocRow>,
    exceptions: Vec<ExcRow>,
    gc_orphans: bool,
}

pub(super) fn severity_label(s: ExceptionSeverity) -> &'static str {
    match s {
        ExceptionSeverity::Info => "info",
        ExceptionSeverity::Warn => "warn",
        ExceptionSeverity::Error => "error",
    }
}

pub(super) async fn bilag_page(
    state: &AppState,
    company: &str,
    gc_orphans: bool,
    flash_ok: String,
    flash_err: String,
) -> BilagTemplate {
    let n = nav("bilag");
    let mut tpl = BilagTemplate {
        title: "Bilag",
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
        company: company.to_string(),
        retain_days: "—".into(),
        purge_after_days: "—".into(),
        documents: Vec::new(),
        exceptions: Vec::new(),
        gc_orphans,
    };
    if company.is_empty() {
        return tpl;
    }
    let path = match authorize_company(state, company).await {
        Ok(p) => p,
        Err(e) => {
            tpl.has_flash_err = true;
            tpl.flash_err = e;
            return tpl;
        }
    };
    if let Ok(policy) = load_retention(&path) {
        tpl.retain_days = policy.retain_days.to_string();
        tpl.purge_after_days = policy
            .purge_closed_exceptions_after_days
            .map(|d| d.to_string())
            .unwrap_or_else(|| "—".into());
    }
    tpl.documents = list_documents(&path)
        .unwrap_or_default()
        .into_iter()
        .map(|d| DocRow {
            id: d.id.to_string(),
            kind: format!("{:?}", d.kind).to_ascii_lowercase(),
            path_hint: d.path_hint,
            notes: d.notes,
        })
        .collect();
    tpl.exceptions = list_exceptions(&path, true)
        .unwrap_or_default()
        .into_iter()
        .map(|e| ExcRow {
            id: e.id.to_string(),
            code: e.code,
            severity: severity_label(e.severity).into(),
            message: e.message,
            status: if e.open { "åben" } else { "lukket" }.into(),
        })
        .collect();
    tpl
}

pub async fn bilag_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    html_ok(bilag_page(&state, &company, false, String::new(), String::new()).await)
}
