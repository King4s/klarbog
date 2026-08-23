//! Home + settings SSR pages.

use askama::Template;
use axum::extract::{Form, State};
use axum::http::header;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use klarbog_core::{load_company_policy, save_company_policy};
use klarbog_types::FiscalYearLabelStrategy;
use serde::Deserialize;
use std::path::Path;

use super::common::{company_from, foot, html_ok, nav, set_company_cookie};
use crate::AppState;

struct PluginRow {
    id: String,
    version: String,
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
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
    mode: String,
    bind: String,
    allowlist_root: String,
    plugins: Vec<PluginRow>,
}

pub async fn home(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    let n = nav("home");
    let plugins = state
        .registry
        .list()
        .map(|p| PluginRow {
            id: p.id().to_string(),
            version: p.version().to_string(),
        })
        .collect();
    html_ok(HomeTemplate {
        title: "Oversigt",
        nav_home: n.home,
        nav_parties: n.parties,
        nav_invoices: n.invoices,
        nav_bank: n.bank,
        nav_bilag: n.bilag,
        nav_journal: n.journal,
        nav_chart: n.chart,
        nav_settings: n.settings,
        foot: foot(&state),
        has_flash_ok: false,
        flash_ok: String::new(),
        has_flash_err: false,
        flash_err: String::new(),
        has_company: !company.is_empty(),
        company,
        mode: "dev".into(),
        bind: "127.0.0.1:3195".into(),
        allowlist_root: state.allowlist_root.display().to_string(),
        plugins,
    })
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
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
    company: String,
    has_policy: bool,
    fiscal_year_start_month: String,
    fiscal_year_label_strategy: String,
}

fn strategy_slug(s: FiscalYearLabelStrategy) -> &'static str {
    match s {
        FiscalYearLabelStrategy::EndYear => "end-year",
        FiscalYearLabelStrategy::StartYear => "start-year",
        FiscalYearLabelStrategy::Span => "span",
    }
}

fn parse_strategy(raw: &str) -> Option<FiscalYearLabelStrategy> {
    match raw.trim() {
        "end-year" => Some(FiscalYearLabelStrategy::EndYear),
        "start-year" => Some(FiscalYearLabelStrategy::StartYear),
        "span" => Some(FiscalYearLabelStrategy::Span),
        _ => None,
    }
}

pub async fn settings_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    let n = nav("settings");
    let (has_policy, fiscal_year_start_month, fiscal_year_label_strategy) =
        if company.trim().is_empty() {
            (false, "1".into(), "end-year".into())
        } else {
            match klarbog_core::assert_company_path(&state.allowlist_root, Path::new(&company)) {
                Ok(path) => match load_company_policy(&path) {
                    Ok(policy) => (
                        true,
                        policy.fiscal_year_start_month.to_string(),
                        strategy_slug(policy.fiscal_year_label_strategy).to_string(),
                    ),
                    Err(_) => (false, "1".into(), "end-year".into()),
                },
                Err(_) => (false, "1".into(), "end-year".into()),
            }
        };
    html_ok(SettingsTemplate {
        title: "Indstillinger",
        nav_home: n.home,
        nav_parties: n.parties,
        nav_invoices: n.invoices,
        nav_bank: n.bank,
        nav_bilag: n.bilag,
        nav_journal: n.journal,
        nav_chart: n.chart,
        nav_settings: n.settings,
        foot: foot(&state),
        has_flash_ok: false,
        flash_ok: String::new(),
        has_flash_err: false,
        flash_err: String::new(),
        company,
        has_policy,
        fiscal_year_start_month,
        fiscal_year_label_strategy,
    })
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub company: String,
    pub fiscal_year_start_month: Option<String>,
    pub fiscal_year_label_strategy: Option<String>,
}

pub async fn settings_post(
    State(state): State<AppState>,
    Form(form): Form<SettingsForm>,
) -> Response {
    let company = form.company.trim().to_string();
    if let Ok(path) = klarbog_core::assert_company_path(&state.allowlist_root, Path::new(&company))
    {
        if let Ok(mut policy) = load_company_policy(&path) {
            if let Some(raw) = form.fiscal_year_start_month.as_deref() {
                if let Ok(month) = raw.trim().parse::<u32>() {
                    if (1..=12).contains(&month) {
                        policy.fiscal_year_start_month = month;
                    }
                }
            }
            if let Some(raw) = form.fiscal_year_label_strategy.as_deref() {
                if let Some(strategy) = parse_strategy(raw) {
                    policy.fiscal_year_label_strategy = strategy;
                }
            }
            let _ = save_company_policy(&path, &policy);
        }
    }
    let mut res = Redirect::to("/ui/settings").into_response();
    res.headers_mut()
        .insert(header::SET_COOKIE, set_company_cookie(&company));
    res
}
