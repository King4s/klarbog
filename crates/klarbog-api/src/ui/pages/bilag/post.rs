//! Bilag POST — exceptions, retention purge, backup.

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_plugin_documents::{raise_exception, ExceptionSeverity};
use klarbog_plugin_retention::{run_retention_purge, write_backup_manifest, PurgeOptions};
use serde::Deserialize;

use super::super::common::{authorize_company, company_from, html_ok};
use super::view::bilag_page;
use crate::AppState;

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
