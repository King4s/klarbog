//! CRM party HTTP handlers (slice 4). AuthZ mirrors journal routes.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_crm::{get_party, list_parties, upsert_party, CrmError, PartyKind};
use klarbog_types::{Actor, Envelope, PartyId};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct UpsertBody {
    pub company: String,
    pub display_name: String,
    pub party_id: Option<String>,
    /// `private` (default) or `business` — billing convention (ADR-020).
    pub kind: Option<String>,
    /// Optional party-specific payment terms; omit to inherit company default.
    pub payment_terms_days: Option<u32>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub company: String,
    pub party_id: Option<String>,
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

fn map_crm(err: CrmError) -> (StatusCode, Envelope<Value>) {
    match err {
        CrmError::NotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("party not found: {id}")]),
        ),
        CrmError::EmptyName => (
            StatusCode::BAD_REQUEST,
            Envelope::err(["display name must not be empty"]),
        ),
        CrmError::InvalidPaymentTerms(days) => (
            StatusCode::BAD_REQUEST,
            Envelope::err([format!(
                "payment_terms_days must be between 1 and 365, got {days}"
            )]),
        ),
        CrmError::Io(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        CrmError::Json(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

pub async fn upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpsertBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = body.party_id.map(PartyId::new);
    let kind = match body.kind.as_deref() {
        None => PartyKind::default(),
        Some(raw) => PartyKind::parse(raw).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(Envelope::err([format!(
                    "kind must be private or business, got {raw:?}"
                )])),
            )
        })?,
    };
    let party =
        upsert_party(&path, id, body.display_name, kind, body.payment_terms_days).map_err(|e| {
            let (s, env) = map_crm(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::to_value(party).unwrap())))
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(query.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    if let Some(raw_id) = query.party_id {
        let id = PartyId::new(raw_id);
        let party = get_party(&path, &id).map_err(|e| {
            let (s, env) = map_crm(e);
            (s, Json(env))
        })?;
        let party = party.ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(Envelope::err([format!("party not found: {id}")])),
            )
        })?;
        return Ok(Json(Envelope::ok(serde_json::to_value(party).unwrap())));
    }
    let parties = list_parties(&path).map_err(|e| {
        let (s, env) = map_crm(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(parties).unwrap())))
}

#[cfg(test)]
mod http_tests {
    use crate::{default_registry, router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{init_company, ConfirmStore};
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
    async fn crm_upsert_list_get() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
            api_token: None,
            session_secret: None,
            session_cookie_secure: false,
        };
        let app = router(state);
        let (kind, id) = actor_headers(&owner);
        let upsert_body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "display_name": "Acme ApS",
        });
        let upsert_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/crm/parties")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::from(upsert_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upsert_res.status(), StatusCode::OK);
        let upsert_bytes = axum::body::to_bytes(upsert_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let upsert_env: Envelope<Value> = serde_json::from_slice(&upsert_bytes).unwrap();
        let party_id = upsert_env.data.unwrap()["id"].as_str().unwrap().to_string();
        let get_uri = format!(
            "/api/v1/crm/parties?company={}&party_id={}",
            company_path.to_string_lossy(),
            party_id,
        );
        let get_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&get_uri)
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_res.status(), StatusCode::OK);
        let list_uri = format!(
            "/api/v1/crm/parties?company={}",
            company_path.to_string_lossy(),
        );
        let list_res = app
            .oneshot(
                Request::builder()
                    .uri(&list_uri)
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn crm_actor_denied() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
            api_token: None,
            session_secret: None,
            session_cookie_secure: false,
        };
        let app = router(state);
        let intruder = Actor::user("intruder");
        let (kind, id) = actor_headers(&intruder);
        let body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "display_name": "Blocked Co",
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/crm/parties")
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
