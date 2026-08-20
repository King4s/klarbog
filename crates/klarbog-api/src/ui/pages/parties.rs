//! Parties SSR pages.

use askama::Template;
use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use klarbog_plugin_crm::{list_parties, upsert_party};
use serde::Deserialize;

use super::common::{authorize_company, company_from, foot, html_ok, nav};
use crate::AppState;

struct PartyRow {
    id: String,
    display_name: String,
}

#[derive(Template)]
#[template(path = "parties.html")]
struct PartiesTemplate {
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
    parties: Vec<PartyRow>,
}

async fn load_parties(state: &AppState, company: &str) -> Result<Vec<PartyRow>, String> {
    if company.is_empty() {
        return Ok(Vec::new());
    }
    let path = authorize_company(state, company).await?;
    let list = list_parties(&path).map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .map(|p| PartyRow {
            id: p.id.to_string(),
            display_name: p.display_name,
        })
        .collect())
}

fn parties_page(
    state: &AppState,
    company: String,
    parties: Vec<PartyRow>,
    flash_err: String,
) -> PartiesTemplate {
    let n = nav("parties");
    let has_err = !flash_err.is_empty();
    PartiesTemplate {
        title: "Parter",
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
        parties,
    }
}

pub async fn parties_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    match load_parties(&state, &company).await {
        Ok(parties) => html_ok(parties_page(&state, company, parties, String::new())),
        Err(e) => html_ok(parties_page(&state, company, Vec::new(), e)),
    }
}

#[derive(Deserialize)]
pub struct PartyForm {
    pub display_name: String,
}

pub async fn parties_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<PartyForm>,
) -> Response {
    let company = company_from(&headers);
    if company.is_empty() {
        return Redirect::to("/ui/settings").into_response();
    }
    let name = form.display_name.trim().to_string();
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => {
            return html_ok(parties_page(&state, company, Vec::new(), e));
        }
    };
    match upsert_party(&path, None, name) {
        Ok(_) => Redirect::to("/ui/parties").into_response(),
        Err(e) => {
            let parties = load_parties(&state, &company).await.unwrap_or_default();
            html_ok(parties_page(&state, company, parties, e.to_string()))
        }
    }
}
