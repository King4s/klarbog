//! Bilag SSR — documents, exceptions, retention purge/backup.

use askama::Template;
use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_plugin_documents::{
    list_documents, list_exceptions, raise_exception, ExceptionSeverity,
};
use klarbog_plugin_retention::{
    load_retention, run_retention_purge, write_backup_manifest, PurgeOptions,
};
use serde::Deserialize;

use super::common::{authorize_company, company_from, foot, html_ok, nav};
use crate::AppState;

struct DocRow {
    id: String,
    kind: String,
    path_hint: String,
    notes: String,
}

struct ExcRow {
    id: String,
    code: String,
    severity: String,
    message: String,
    status: String,
}

#[derive(Template)]
#[template(path = "bilag.html")]
struct BilagTemplate {
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

fn severity_label(s: ExceptionSeverity) -> &'static str {
    match s {
        ExceptionSeverity::Info => "info",
        ExceptionSeverity::Warn => "warn",
        ExceptionSeverity::Error => "error",
    }
}

async fn bilag_page(
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

#[derive(Deserialize)]
pub struct BilagActionForm {
    pub action: String,
    pub code: Option<String>,
    pub severity: Option<String>,
    pub message: Option<String>,
    pub gc_orphan_documents: Option<String>,
}

fn parse_severity(raw: &str) -> ExceptionSeverity {
    match raw.trim() {
        "info" => ExceptionSeverity::Info,
        "error" => ExceptionSeverity::Error,
        _ => ExceptionSeverity::Warn,
    }
}

pub async fn bilag_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<BilagActionForm>,
) -> Response {
    let company = company_from(&headers);
    let gc = form.gc_orphan_documents.is_some();
    if company.is_empty() {
        return html_ok(
            bilag_page(
                &state,
                &company,
                gc,
                String::new(),
                "Sæt firmasti under Indstillinger.".into(),
            )
            .await,
        );
    }
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => {
            return html_ok(bilag_page(&state, &company, gc, String::new(), e).await);
        }
    };
    match form.action.trim() {
        "raise_exception" => match raise_exception(
            &path,
            form.code.unwrap_or_default().trim().to_string(),
            parse_severity(&form.severity.unwrap_or_default()),
            form.message.unwrap_or_default().trim().to_string(),
            Vec::new(),
        ) {
            Ok(exc) => html_ok(
                bilag_page(
                    &state,
                    &company,
                    gc,
                    format!("Undtagelse oprettet: {}", exc.id),
                    String::new(),
                )
                .await,
            ),
            Err(e) => html_ok(bilag_page(&state, &company, gc, String::new(), e.to_string()).await),
        },
        "purge_dry" => match run_retention_purge(
            &path,
            PurgeOptions {
                confirm: false,
                gc_orphan_documents: gc,
            },
        )
        .await
        {
            Ok(r) => html_ok(
                bilag_page(
                    &state,
                    &company,
                    gc,
                    format!(
                        "Dry-run: {} undtagelser · {} orphan docs",
                        r.exceptions_purged.len(),
                        r.orphan_documents_gc.len()
                    ),
                    String::new(),
                )
                .await,
            ),
            Err(e) => html_ok(bilag_page(&state, &company, gc, String::new(), e.to_string()).await),
        },
        "purge_confirm" => match run_retention_purge(
            &path,
            PurgeOptions {
                confirm: true,
                gc_orphan_documents: gc,
            },
        )
        .await
        {
            Ok(r) => html_ok(
                bilag_page(
                    &state,
                    &company,
                    gc,
                    format!(
                        "Purge udført: {} undtagelser · {} orphan docs",
                        r.exceptions_purged.len(),
                        r.orphan_documents_gc.len()
                    ),
                    String::new(),
                )
                .await,
            ),
            Err(e) => html_ok(bilag_page(&state, &company, gc, String::new(), e.to_string()).await),
        },
        "backup" => match write_backup_manifest(&path).await {
            Ok(m) => html_ok(
                bilag_page(
                    &state,
                    &company,
                    gc,
                    format!("Backup manifest: {}", m.backup_key),
                    String::new(),
                )
                .await,
            ),
            Err(e) => html_ok(bilag_page(&state, &company, gc, String::new(), e.to_string()).await),
        },
        _ => html_ok(
            bilag_page(
                &state,
                &company,
                gc,
                String::new(),
                format!("Ukendt handling: {}", form.action),
            )
            .await,
        ),
    }
}
