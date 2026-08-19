//! Bank CSV import preview (ADR-006). Read-only — no journal post.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_bank::{
    draft_entries_from_rows, parse_bank_csv_with_profile, BankCsvError, BankImportConfig,
    BankMapError, BankProfile,
};
use klarbog_types::{Actor, Currency, Envelope};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct PreviewBody {
    pub company: String,
    pub profile: BankProfile,
    pub csv: String,
    pub currency: Option<String>,
}

#[derive(serde::Serialize)]
struct DraftSummary {
    memo: String,
    amount_minor: i64,
}

async fn authorize_company(
    allowlist_root: &Path,
    company: &Path,
    actor: &Actor,
) -> Result<PathBuf, CoreError> {
    let path = assert_company_path(allowlist_root, company)?;
    open_existing(&path).await?.authorize(actor)?;
    Ok(path)
}

fn map_core(err: CoreError) -> (StatusCode, Envelope<Value>) {
    match err {
        CoreError::ActorDenied(tag) => (
            StatusCode::FORBIDDEN,
            Envelope::err([format!("actor not in policy: {tag}")]),
        ),
        CoreError::Path(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::Store(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        CoreError::Other(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([other.to_string()]),
        ),
    }
}

fn map_csv(err: BankCsvError) -> (StatusCode, Envelope<Value>) {
    (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
}

fn map_map(err: BankMapError) -> (StatusCode, Envelope<Value>) {
    (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
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
    let _path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let cfg = BankImportConfig {
        currency: currency.clone(),
        ..BankImportConfig::default()
    };
    let required = match body.profile {
        BankProfile::Revolut => Some(currency),
        BankProfile::GenericDk => None,
    };
    let rows =
        parse_bank_csv_with_profile(body.profile, &body.csv, required.as_ref()).map_err(|e| {
            let (s, env) = map_csv(e);
            (s, Json(env))
        })?;
    let drafts = draft_entries_from_rows(&rows, &cfg, &actor).map_err(|e| {
        let (s, env) = map_map(e);
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
        "drafts": summaries,
    }))))
}

#[cfg(test)]
mod http_tests {
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_types::{Actor, ActorKind, Envelope};
    use serde_json::Value;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tower::ServiceExt;

    const FIXTURE: &str = "Dato;Tekst;Beløb\n19.08.2026;Office supplies;-125,50\n20.08.2026;Customer payment;500,00\n";

    fn actor_headers(actor: &Actor) -> (&'static str, String) {
        let kind = match actor.kind {
            ActorKind::User => "user",
            ActorKind::Agent => "agent",
            ActorKind::System => "system",
        };
        (kind, actor.id.clone())
    }

    #[tokio::test]
    async fn bank_preview_generic_dk() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
        };
        let app = router(state);
        let (kind, id) = actor_headers(&owner);
        let body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "profile": "generic_dk",
            "csv": FIXTURE,
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/bank/import/preview")
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
        assert_eq!(data["count"], 2);
        assert_eq!(data["drafts"][0]["amount_minor"], -12550);
        assert!(data["drafts"][0]["memo"]
            .as_str()
            .unwrap()
            .contains("Office supplies"));
    }

    #[tokio::test]
    async fn bank_preview_actor_denied() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
        };
        let app = router(state);
        let intruder = Actor::user("intruder");
        let (kind, id) = actor_headers(&intruder);
        let body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "profile": "generic_dk",
            "csv": FIXTURE,
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/bank/import/preview")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}
