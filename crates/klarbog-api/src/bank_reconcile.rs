//! POST /api/v1/bank/reconcile/suggest — match bank rows to open invoice drafts.

use crate::actor::parse_actor;
use crate::bank::{authorize_company, map_core, map_import};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use klarbog_plugin_bank::{
    default_source_for_rail, import_preview, list_unmatched_bank_exceptions, suggest_matches,
    sync_unmatched_exceptions, BankImportConfig, BankImportSource, BankProfile, BankRow,
    ReconcileError,
};
use klarbog_types::{Actor, Currency, Envelope, MinorAmount};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct ParsedBankRowInput {
    pub date: String,
    pub text: String,
    pub amount_minor: i64,
}

#[derive(Deserialize)]
pub struct ReconcileSuggestBody {
    pub company: String,
    #[serde(alias = "profile")]
    pub provider: Option<BankProfile>,
    pub source: Option<BankImportSource>,
    pub csv: Option<String>,
    pub currency: Option<String>,
    pub rows: Option<Vec<ParsedBankRowInput>>,
    #[serde(default = "default_raise_exceptions")]
    pub raise_exceptions: bool,
}

fn default_raise_exceptions() -> bool {
    true
}

fn parse_row_input(input: &ParsedBankRowInput) -> Result<BankRow, String> {
    let date = NaiveDate::parse_from_str(&input.date, "%Y-%m-%d")
        .map_err(|e| format!("invalid date {}: {e}", input.date))?;
    let dt: DateTime<Utc> = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    Ok(BankRow {
        date: dt,
        text: input.text.clone(),
        amount_minor: MinorAmount::from_minor(input.amount_minor),
    })
}

async fn resolve_bank_rows(
    company: &Path,
    body: &ReconcileSuggestBody,
    cfg: &BankImportConfig,
    actor: &Actor,
) -> Result<Vec<BankRow>, (StatusCode, Envelope<Value>)> {
    if let Some(ref parsed) = body.rows {
        let mut out = Vec::with_capacity(parsed.len());
        for input in parsed {
            out.push(
                parse_row_input(input)
                    .map_err(|e| (StatusCode::BAD_REQUEST, Envelope::err([e])))?,
            );
        }
        return Ok(out);
    }
    let provider = body.provider.ok_or((
        StatusCode::BAD_REQUEST,
        Envelope::err(["missing provider or rows"]),
    ))?;
    let source = body
        .source
        .unwrap_or_else(|| default_source_for_rail(provider));
    if matches!(source, BankImportSource::Csv) && body.csv.as_deref().unwrap_or("").is_empty() {
        return Err((StatusCode::BAD_REQUEST, Envelope::err(["missing csv"])));
    }
    import_preview(
        source,
        provider,
        body.csv.as_deref(),
        cfg,
        actor,
        Some(company),
    )
    .await
    .map(|(rows, _)| rows)
    .map_err(|e| {
        let (s, env) = map_import(e);
        (s, env)
    })
}

fn map_reconcile(err: ReconcileError) -> (StatusCode, Envelope<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Envelope::err([err.to_string()]),
    )
}

pub async fn reconcile_suggest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ReconcileSuggestBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let currency_code = body.currency.as_deref().unwrap_or("DKK");
    let currency = Currency::new(currency_code).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(Envelope::err([e.to_string()])),
        )
    })?;
    let company = PathBuf::from(body.company.clone());
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
    let rows = resolve_bank_rows(&path, &body, &cfg, &actor)
        .await
        .map_err(|(s, e)| (s, Json(e)))?;
    let match_results = suggest_matches(&path, &rows).map_err(|e| {
        let (s, env) = map_reconcile(e);
        (s, Json(env))
    })?;
    let raised = if body.raise_exceptions {
        sync_unmatched_exceptions(&path, &rows, &match_results).map_err(|e| {
            let (s, env) = map_reconcile(e);
            (s, Json(env))
        })?
    } else {
        Vec::new()
    };
    let open_unmatched = list_unmatched_bank_exceptions(&path).map_err(|e| {
        let (s, env) = map_reconcile(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "count": match_results.len(),
        "rows": match_results,
        "exceptions_raised": raised.iter().map(|e| e.id.to_string()).collect::<Vec<_>>(),
        "exceptions": open_unmatched,
    }))))
}

#[cfg(test)]
mod http_tests {
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
    use klarbog_types::{Actor, ActorKind, Envelope};
    use serde_json::Value;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tower::ServiceExt;

    fn actor_headers(actor: &Actor) -> (&'static str, String) {
        let kind = match actor.kind {
            ActorKind::User => "user",
            ActorKind::Agent => "agent",
            ActorKind::System => "system",
        };
        (kind, actor.id.clone())
    }

    #[tokio::test]
    async fn reconcile_suggest_with_parsed_rows() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let party = upsert_party(&company_path, None, "Nordic Supply".into()).unwrap();
        create_draft_from_new(
            &company_path,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Widgets".into(),
                amount_minor: 50_000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
        };
        let app = router(state);
        let (kind, id) = actor_headers(&owner);
        let body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "rows": [{
                "date": "2026-05-20",
                "text": "Customer payment Nordic Supply consulting",
                "amount_minor": 50000
            }]
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/bank/reconcile/suggest")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
        let data = env.data.unwrap();
        assert_eq!(data["count"], 1);
        assert_eq!(data["rows"][0]["suggestions"].as_array().unwrap().len(), 1);
    }
}
