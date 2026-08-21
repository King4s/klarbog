//! Persist a CSV import with batch trail (DK-BOOKKEEPING-BANK-IMPORT-001).

use axum::response::Response;
use chrono::Utc;
use klarbog_plugin_bank::{
    commit_import, default_source_for_rail, import_preview, BankImportConfig, BankProfile,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::BankActionForm;
use super::view::{bank_page, BankView};
use crate::AppState;

#[allow(clippy::too_many_arguments)]
pub(super) async fn import_commit(
    state: &AppState,
    company: String,
    path: &std::path::Path,
    actor: &Actor,
    provider: BankProfile,
    provider_s: String,
    cfg: &BankImportConfig,
    form: &BankActionForm,
) -> Response {
    let err_page = |msg: String| {
        bank_page(
            state,
            BankView {
                company: company.clone(),
                provider: provider_s.clone(),
                csv: form.csv.clone(),
                flash_err: msg,
                ..Default::default()
            },
        )
    };
    if form.csv.trim().is_empty() {
        return html_ok(err_page("CSV kræves til import-commit.".into()));
    }
    let source = default_source_for_rail(provider);
    let rows = match import_preview(
        source,
        provider,
        Some(form.csv.trim()),
        cfg,
        actor,
        Some(path),
    )
    .await
    {
        Ok((rows, _)) => rows,
        Err(e) => return html_ok(err_page(e.to_string())),
    };
    match commit_import(path, form.csv.as_bytes(), &rows, Utc::now()) {
        Ok(r) => html_ok(bank_page(
            state,
            BankView {
                company,
                provider: provider_s,
                csv: form.csv.clone(),
                flash_ok: format!(
                    "Import gemt · batch {} · {} nye · {} dubletter sprunget over · hash {}…",
                    r.import_batch_id,
                    r.imported,
                    r.skipped_duplicates,
                    &r.source_file_hash[..8]
                ),
                ..Default::default()
            },
        )),
        Err(e) => html_ok(err_page(e.to_string())),
    }
}
