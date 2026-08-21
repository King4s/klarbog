//! Chart SSR page.

use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_plugin_rules_dk::{chart_stub_entries, RULE_KNOWN_ACCOUNT};

use super::common::{authorize_company, company_from, foot, format_dkk, html_ok, nav};
use crate::AppState;

struct AccountRow {
    code: String,
    label: String,
    range: String,
}

struct BalanceRow {
    account: String,
    debit: String,
    credit: String,
    net: String,
}

#[derive(Template)]
#[template(path = "chart.html")]
struct ChartTemplate {
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
    stub: bool,
    rule_known_account: String,
    accounts: Vec<AccountRow>,
    balances: Vec<BalanceRow>,
    total_debit: String,
    total_credit: String,
}

fn chart_page(
    state: &AppState,
    company: String,
    flash_err: String,
    accounts: Vec<AccountRow>,
) -> ChartTemplate {
    let n = nav("chart");
    let has_err = !flash_err.is_empty();
    ChartTemplate {
        title: "Kontoplan",
        nav_home: n.home,
        nav_parties: n.parties,
        nav_invoices: n.invoices,
        nav_bank: n.bank,
        nav_bilag: n.bilag,
        nav_journal: n.journal,
        nav_chart: n.chart,
        nav_settings: n.settings,
        foot: foot(state),
        has_flash_ok: false,
        flash_ok: String::new(),
        has_flash_err: has_err,
        flash_err,
        has_company: !company.is_empty(),
        company,
        stub: true,
        rule_known_account: RULE_KNOWN_ACCOUNT.to_string(),
        accounts,
        balances: Vec::new(),
        total_debit: format_dkk(0),
        total_credit: format_dkk(0),
    }
}

/// Balance rows plus råbalance totals (double-entry invariant: debet == kredit).
async fn load_balances(path: &std::path::Path) -> Result<(Vec<BalanceRow>, i64, i64), String> {
    let company = klarbog_core::open_existing(path)
        .await
        .map_err(|e| e.to_string())?;
    let balances = company
        .account_balances()
        .await
        .map_err(|e| e.to_string())?;
    let total_debit: i64 = balances.iter().map(|b| b.debit_minor).sum();
    let total_credit: i64 = balances.iter().map(|b| b.credit_minor).sum();
    let rows = balances
        .into_iter()
        .map(|b| BalanceRow {
            account: b.account.clone(),
            debit: format_dkk(b.debit_minor),
            credit: format_dkk(b.credit_minor),
            net: format_dkk(b.net_minor()),
        })
        .collect();
    Ok((rows, total_debit, total_credit))
}

pub async fn chart_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    if company.is_empty() {
        return html_ok(chart_page(&state, company, String::new(), Vec::new()));
    }
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => return html_ok(chart_page(&state, company, e, Vec::new())),
    };
    let accounts = chart_stub_entries()
        .into_iter()
        .map(|a| {
            let range = match (a.min, a.max) {
                (Some(min), Some(max)) => format!("{min}–{max}"),
                _ => a.code.to_string(),
            };
            AccountRow {
                code: a.code.to_string(),
                label: a.label.to_string(),
                range,
            }
        })
        .collect();
    let page = match load_balances(&path).await {
        Ok((balances, total_debit, total_credit)) => {
            let mut p = chart_page(&state, company, String::new(), accounts);
            p.balances = balances;
            p.total_debit = format_dkk(total_debit);
            p.total_credit = format_dkk(total_credit);
            p
        }
        Err(e) => chart_page(&state, company, e, accounts),
    };
    html_ok(page)
}
