//! Chart page rendering (GET) and shared loaders.

use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_plugin_rules_dk::{chart_accounts, AccountType, RULE_KNOWN_ACCOUNT};

use super::super::common::{authorize_company, company_from, foot, format_dkk, html_ok, nav};
use super::{KOEBSMOMS, SALGSMOMS};
use crate::AppState;

pub(super) struct AccountRow {
    code: String,
    label: String,
    kind: &'static str,
    normal: &'static str,
}

fn kind_label(t: AccountType) -> &'static str {
    match t {
        AccountType::Income => "Indtægt",
        AccountType::Expense => "Omkostning",
        AccountType::Asset => "Aktiv",
        AccountType::Liability => "Passiv",
        AccountType::Equity => "Egenkapital",
        AccountType::Vat => "Moms",
    }
}

pub(super) struct BalanceRow {
    account: String,
    debit: String,
    credit: String,
    net: String,
}

/// Net VAT position: salgsmoms (net credit), købsmoms (net debit).
pub(super) struct MomsPosition {
    pub salgs_minor: i64,
    pub koebs_minor: i64,
}

impl MomsPosition {
    pub fn net_due_minor(&self) -> i64 {
        self.salgs_minor - self.koebs_minor
    }
    pub fn has_anything(&self) -> bool {
        self.salgs_minor != 0 || self.koebs_minor != 0
    }
}

#[derive(Template)]
#[template(path = "chart.html")]
pub(super) struct ChartTemplate {
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
    pub rule_known_account: String,
    pub accounts: Vec<AccountRow>,
    pub balances: Vec<BalanceRow>,
    pub total_debit: String,
    pub total_credit: String,
    pub moms_salgs: String,
    pub moms_koebs: String,
    pub moms_net: String,
    pub moms_net_label: &'static str,
    pub can_settle: bool,
    pub has_preview: bool,
    pub confirm_token: String,
    pub expires_unix_ms: String,
    pub payload_digest: String,
    pub entry_json: String,
}

pub(super) fn chart_page(state: &AppState, company: String, flash_err: String) -> ChartTemplate {
    let n = nav("chart");
    let has_err = !flash_err.is_empty();
    let accounts = chart_accounts()
        .iter()
        .map(|a| AccountRow {
            code: a.code.to_string(),
            label: a.label.to_string(),
            kind: kind_label(a.account_type),
            normal: if a.credit_normal { "Kredit" } else { "Debet" },
        })
        .collect();
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
        rule_known_account: RULE_KNOWN_ACCOUNT.to_string(),
        accounts,
        balances: Vec::new(),
        total_debit: format_dkk(0),
        total_credit: format_dkk(0),
        moms_salgs: format_dkk(0),
        moms_koebs: format_dkk(0),
        moms_net: format_dkk(0),
        moms_net_label: "Skyldig moms",
        can_settle: false,
        has_preview: false,
        confirm_token: String::new(),
        expires_unix_ms: String::new(),
        payload_digest: String::new(),
        entry_json: String::new(),
    }
}

/// Fill balances, råbalance totals and the VAT position onto the page.
pub(super) async fn load_ledger(
    page: &mut ChartTemplate,
    path: &std::path::Path,
) -> Result<MomsPosition, String> {
    let company = klarbog_core::open_existing(path)
        .await
        .map_err(|e| e.to_string())?;
    let balances = company
        .account_balances()
        .await
        .map_err(|e| e.to_string())?;
    page.total_debit = format_dkk(balances.iter().map(|b| b.debit_minor).sum());
    page.total_credit = format_dkk(balances.iter().map(|b| b.credit_minor).sum());
    let moms = MomsPosition {
        salgs_minor: balances
            .iter()
            .find(|b| b.account == SALGSMOMS)
            .map(|b| b.credit_minor - b.debit_minor)
            .unwrap_or(0),
        koebs_minor: balances
            .iter()
            .find(|b| b.account == KOEBSMOMS)
            .map(|b| b.debit_minor - b.credit_minor)
            .unwrap_or(0),
    };
    page.balances = balances
        .into_iter()
        .map(|b| BalanceRow {
            account: b.account.clone(),
            debit: format_dkk(b.debit_minor),
            credit: format_dkk(b.credit_minor),
            net: format_dkk(b.net_minor()),
        })
        .collect();
    page.moms_salgs = format_dkk(moms.salgs_minor);
    page.moms_koebs = format_dkk(moms.koebs_minor);
    let net = moms.net_due_minor();
    page.moms_net = format_dkk(net.abs());
    page.moms_net_label = if net < 0 {
        "Momstilgodehavende"
    } else {
        "Skyldig moms"
    };
    page.can_settle = moms.has_anything();
    Ok(moms)
}

pub async fn chart_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    if company.is_empty() {
        return html_ok(chart_page(&state, company, String::new()));
    }
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => return html_ok(chart_page(&state, company, e)),
    };
    let mut page = chart_page(&state, company, String::new());
    if let Err(e) = load_ledger(&mut page, &path).await {
        page.has_flash_err = true;
        page.flash_err = e;
    }
    html_ok(page)
}
