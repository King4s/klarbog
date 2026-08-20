//! Parties SSR pages — CRM upsert plus GDPR erase (dry-run → confirm).

use askama::Template;
use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use klarbog_plugin_crm::{list_parties, upsert_party};
use klarbog_plugin_retention::{erase_party, ErasePartyOptions, ErasePartyReport};
use klarbog_types::PartyId;
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
    has_report: bool,
    report_party: String,
    report_lines: Vec<String>,
    report_delete_docs: bool,
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

fn report_lines(report: &ErasePartyReport) -> Vec<String> {
    vec![
        format!(
            "Navn: {} → {}",
            report.display_name_before, report.display_name_after
        ),
        format!("Dokumenter strippet: {}", report.documents_stripped.len()),
        format!("Dokumenter slettet: {}", report.documents_deleted.len()),
        format!(
            "Journal-referencer bevares (immutable): {}",
            report.journal_refs_retained.len()
        ),
    ]
}

fn parties_page(
    state: &AppState,
    company: String,
    parties: Vec<PartyRow>,
    flash_ok: String,
    flash_err: String,
) -> PartiesTemplate {
    let n = nav("parties");
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
        has_flash_ok: !flash_ok.is_empty(),
        flash_ok,
        has_flash_err: !flash_err.is_empty(),
        flash_err,
        has_company: !company.is_empty(),
        company,
        parties,
        has_report: false,
        report_party: String::new(),
        report_lines: Vec::new(),
        report_delete_docs: false,
    }
}

pub async fn parties_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    match load_parties(&state, &company).await {
        Ok(parties) => html_ok(parties_page(
            &state,
            company,
            parties,
            String::new(),
            String::new(),
        )),
        Err(e) => html_ok(parties_page(&state, company, Vec::new(), String::new(), e)),
    }
}

#[derive(Deserialize)]
pub struct PartyForm {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub party_id: String,
    #[serde(default)]
    pub delete_documents: String,
}

fn wants_delete_docs(raw: &str) -> bool {
    matches!(raw.trim(), "on" | "1" | "true")
}

async fn page_err(state: &AppState, company: String, e: String) -> Response {
    let parties = load_parties(state, &company).await.unwrap_or_default();
    html_ok(parties_page(state, company, parties, String::new(), e))
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
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => return page_err(&state, company, e).await,
    };
    match form.action.trim() {
        "" | "create" => {
            let name = form.display_name.trim().to_string();
            match upsert_party(&path, None, name) {
                Ok(_) => Redirect::to("/ui/parties").into_response(),
                Err(e) => page_err(&state, company, e.to_string()).await,
            }
        }
        "erase_dry" | "erase_confirm" => {
            let confirm = form.action.trim() == "erase_confirm";
            let delete_documents = wants_delete_docs(&form.delete_documents);
            let party_id = PartyId::new(form.party_id.trim().to_string());
            match erase_party(
                &path,
                &party_id,
                ErasePartyOptions {
                    confirm,
                    delete_documents,
                },
            )
            .await
            {
                Ok(report) => {
                    let parties = load_parties(&state, &company).await.unwrap_or_default();
                    let flash_ok = if confirm {
                        format!("GDPR-slet udført for {party_id}")
                    } else {
                        format!("GDPR dry-run for {party_id} — bekræft nedenfor")
                    };
                    let mut page = parties_page(&state, company, parties, flash_ok, String::new());
                    page.has_report = !confirm;
                    page.report_party = party_id.to_string();
                    page.report_lines = report_lines(&report);
                    page.report_delete_docs = delete_documents;
                    html_ok(page)
                }
                Err(e) => page_err(&state, company, e.to_string()).await,
            }
        }
        other => page_err(&state, company, format!("Ukendt handling: {other}")).await,
    }
}
