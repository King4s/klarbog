//! Invoices page render helpers.

use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_plugin_crm::list_parties;
use klarbog_plugin_invoice::{list_invoices, InvoiceStatus};

use super::super::common::{authorize_company, company_from, foot, format_dkk, html_ok, nav};
use super::form::{status_label, InvoiceRow, PartyOption};
use crate::AppState;

#[derive(Template)]
#[template(path = "invoices.html")]
pub(super) struct InvoicesTemplate {
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
    pub invoices: Vec<InvoiceRow>,
    pub parties: Vec<PartyOption>,
}

pub(super) async fn load_page(
    state: &AppState,
    company: &str,
    flash_ok: String,
    flash_err: String,
) -> InvoicesTemplate {
    let n = nav("invoices");
    let base = InvoicesTemplate {
        title: "Fakturaer",
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
        invoices: Vec::new(),
        parties: Vec::new(),
    };
    if company.is_empty() {
        return base;
    }
    let path = match authorize_company(state, company).await {
        Ok(p) => p,
        Err(e) => {
            return InvoicesTemplate {
                has_flash_err: true,
                flash_err: e,
                ..base
            };
        }
    };
    let parties = list_parties(&path)
        .unwrap_or_default()
        .into_iter()
        .map(|p| PartyOption {
            id: p.id.to_string(),
            label: format!("{} ({})", p.display_name, p.id),
        })
        .collect();
    let invoices = list_invoices(&path)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|inv| {
            let total = inv.total_minor().ok()?;
            Some(InvoiceRow {
                id: inv.id.to_string(),
                status: status_label(inv.status).into(),
                total: format_dkk(total),
                party_id: inv.party_id.to_string(),
                can_send: inv.status == InvoiceStatus::Draft,
                can_collect: inv.status.allows_mark_paid(),
            })
        })
        .collect();
    InvoicesTemplate {
        parties,
        invoices,
        ..base
    }
}

pub async fn invoices_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    html_ok(load_page(&state, &company, String::new(), String::new()).await)
}
