//! Bank import preview (ADR-006/008/009). Read-only — no journal post.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_bank::{
    default_source_for_rail, import_preview, BankImportConfig, BankImportError, BankImportSource,
    BankProfile,
};
use klarbog_types::{Actor, Currency, Envelope};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct PreviewBody {
    pub company: String,
    #[serde(alias = "profile")]
    pub provider: BankProfile,
    pub source: Option<BankImportSource>,
    pub csv: Option<String>,
    pub currency: Option<String>,
}

#[derive(serde::Serialize)]
struct DraftSummary {
    memo: String,
    amount_minor: i64,
}

pub(crate) async fn authorize_company(
    allowlist_root: &Path,
    company: &Path,
    actor: &Actor,
) -> Result<PathBuf, CoreError> {
    let path = assert_company_path(allowlist_root, company)?;
    open_existing(&path).await?.authorize(actor)?;
    Ok(path)
}

pub(crate) fn map_core(err: CoreError) -> (StatusCode, Envelope<Value>) {
    match err {
        CoreError::ActorDenied(tag) => (
            StatusCode::FORBIDDEN,
            Envelope::err([format!("actor not in policy: {tag}")]),
        ),
        CoreError::Path(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::Journal(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::RulesViolation(msg) => (StatusCode::BAD_REQUEST, Envelope::err([msg])),
        CoreError::Confirm(klarbog_types::KlarbogError::ConfirmRequired) => (
            StatusCode::BAD_REQUEST,
            Envelope::err(["confirm token required or already consumed"]),
        ),
        CoreError::Confirm(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::Store(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        CoreError::Other(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

pub(crate) fn map_import(err: BankImportError) -> (StatusCode, Envelope<Value>) {
    let status = match &err {
        BankImportError::Config(_) => StatusCode::SERVICE_UNAVAILABLE,
        BankImportError::MissingCsv | BankImportError::ApiNotSupported { .. } => {
            StatusCode::BAD_REQUEST
        }
        BankImportError::Csv(_) | BankImportError::Map(_) => StatusCode::BAD_REQUEST,
        BankImportError::OAuth(_) => StatusCode::SERVICE_UNAVAILABLE,
        BankImportError::Api(e) => match e {
            klarbog_plugin_bank::BankApiError::Http { status, .. } if *status == 401 => {
                StatusCode::BAD_GATEWAY
            }
            klarbog_plugin_bank::BankApiError::Http { .. } => StatusCode::BAD_GATEWAY,
            _ => StatusCode::BAD_GATEWAY,
        },
    };
    (status, Envelope::err([err.to_string()]))
}

fn resolve_currency(body: &PreviewBody) -> Result<Currency, (StatusCode, Envelope<Value>)> {
    let code = body.currency.as_deref().unwrap_or("DKK");
    Currency::new(code).map_err(|e| (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])))
}

pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PreviewBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let currency = resolve_currency(&body).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let cfg = BankImportConfig {
        currency: currency.clone(),
        ..BankImportConfig::default()
    };
    let source = body
        .source
        .unwrap_or_else(|| default_source_for_rail(body.provider));
    if matches!(source, BankImportSource::Csv) && body.csv.as_deref().unwrap_or("").is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(Envelope::err(["missing csv"])),
        ));
    }
    let (rows, drafts) = import_preview(
        source,
        body.provider,
        body.csv.as_deref(),
        &cfg,
        &actor,
        Some(&path),
    )
    .await
    .map_err(|e| {
        let (s, env) = map_import(e);
        (s, Json(env))
    })?;
    let summaries: Vec<DraftSummary> = rows
        .iter()
        .zip(drafts.iter())
        .map(|(row, entry)| DraftSummary {
            memo: entry.memo.clone(),
            amount_minor: row.amount_minor.minor(),
        })
        .collect();
    Ok(Json(Envelope::ok(serde_json::json!({
        "count": summaries.len(),
        "source": source,
        "provider": body.provider,
        "drafts": summaries,
    }))))
}

#[cfg(test)]
#[path = "bank_http_env_tests.rs"]
mod http_env_tests;
#[cfg(test)]
#[path = "bank_http_tests.rs"]
mod http_tests;
