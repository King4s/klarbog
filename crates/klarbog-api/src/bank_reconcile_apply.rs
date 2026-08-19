//! POST /api/v1/bank/reconcile/apply — journal preview suggestion only (no post).

use crate::actor::parse_actor;
use crate::bank::{authorize_company, map_core, map_import};
use crate::bank_reconcile::{parse_row_input, ParsedBankRowInput};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_bank::{
    apply_match, default_source_for_rail, import_preview, BankImportConfig, BankImportSource,
    BankProfile, BankRow, ReconcileError,
};
use klarbog_types::{Actor, Currency, Envelope};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct ReconcileApplyBody {
    pub company: String,
    pub invoice_id: String,
    pub row: Option<ParsedBankRowInput>,
    pub row_index: Option<usize>,
    #[serde(alias = "profile")]
    pub provider: Option<BankProfile>,
    pub source: Option<BankImportSource>,
    pub csv: Option<String>,
    pub currency: Option<String>,
    pub rows: Option<Vec<ParsedBankRowInput>>,
    #[serde(default)]
    pub force: bool,
}

fn map_reconcile(err: ReconcileError) -> (StatusCode, Envelope<Value>) {
    let status = match &err {
        ReconcileError::InvoiceNotFound(_) => StatusCode::NOT_FOUND,
        ReconcileError::InvoiceNotOpen(_)
        | ReconcileError::AmountMismatch
        | ReconcileError::UnsafeMatch { .. } => StatusCode::BAD_REQUEST,
        ReconcileError::ForceRequiresUser => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Envelope::err([err.to_string()]))
}

async fn resolve_apply_row(
    company: &Path,
    body: &ReconcileApplyBody,
    cfg: &BankImportConfig,
    actor: &Actor,
) -> Result<(BankRow, usize), (StatusCode, Envelope<Value>)> {
    if let Some(ref input) = body.row {
        let row =
            parse_row_input(input).map_err(|e| (StatusCode::BAD_REQUEST, Envelope::err([e])))?;
        return Ok((row, body.row_index.unwrap_or(0)));
    }
    let rows = if let Some(ref parsed) = body.rows {
        let mut out = Vec::with_capacity(parsed.len());
        for input in parsed {
            out.push(
                parse_row_input(input)
                    .map_err(|e| (StatusCode::BAD_REQUEST, Envelope::err([e])))?,
            );
        }
        out
    } else {
        let provider = body.provider.ok_or((
            StatusCode::BAD_REQUEST,
            Envelope::err(["missing row, rows, or provider"]),
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
        })?
    };
    let idx = body.row_index.ok_or((
        StatusCode::BAD_REQUEST,
        Envelope::err(["missing row_index when using rows/csv"]),
    ))?;
    let row = rows.get(idx).cloned().ok_or((
        StatusCode::BAD_REQUEST,
        Envelope::err([format!("row_index {idx} out of range")]),
    ))?;
    Ok((row, idx))
}

pub async fn reconcile_apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ReconcileApplyBody>,
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
    let (row, row_index) = resolve_apply_row(&path, &body, &cfg, &actor)
        .await
        .map_err(|(s, e)| (s, Json(e)))?;
    let applied = apply_match(&path, &row, &body.invoice_id, &actor, body.force, row_index)
        .map_err(|e| {
            let (s, env) = map_reconcile(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "entry": applied.entry,
        "confidence_bps": applied.confidence_bps,
        "forced": applied.forced,
        "exception_closed": applied.exception_closed.as_ref().map(|e| e.id.to_string()),
        "invoice_id": body.invoice_id,
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
    async fn reconcile_apply_returns_entry_json() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let party = upsert_party(&company_path, None, "Nordic Supply".into()).unwrap();
        let inv = create_draft_from_new(
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
            "invoice_id": inv.id.to_string(),
            "row": {
                "date": "2026-05-20",
                "text": "Customer payment Nordic Supply consulting",
                "amount_minor": 50000
            }
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/bank/reconcile/apply")
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
        assert!(data["entry"]["memo"].as_str().unwrap().contains("bank:"));
        assert!(data["entry"]["legs"].as_array().unwrap()[0]["party_id"].is_string());
        assert_eq!(data["forced"], false);
    }
}
