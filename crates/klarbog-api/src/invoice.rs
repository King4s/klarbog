//! Invoice draft HTTP handlers (slice 6). AuthZ mirrors CRM routes.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_invoice::{
    create_draft_from_new, get_invoice, journal_suggestion, list_invoices, InvoiceConfig,
    InvoiceError, InvoiceId, InvoiceKind, NewLine,
};
use klarbog_types::{Actor, Envelope, PartyId};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct CreateBody {
    pub company: String,
    pub party_id: String,
    pub kind: InvoiceKind,
    pub lines: Vec<NewLine>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub company: String,
    pub invoice_id: Option<String>,
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

pub(crate) fn map_invoice(err: InvoiceError) -> (StatusCode, Envelope<Value>) {
    match err {
        InvoiceError::NotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("invoice not found: {id}")]),
        ),
        InvoiceError::PartyNotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("party not found: {id}")]),
        ),
        InvoiceError::NoLines
        | InvoiceError::EmptyDescription
        | InvoiceError::NonPositiveAmount
        | InvoiceError::MixedCurrency => {
            (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
        }
        InvoiceError::Io(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        InvoiceError::Json(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        InvoiceError::Overflow | InvoiceError::Money(_) | InvoiceError::Journal(_) => {
            (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
        }
        InvoiceError::InvalidTransition { .. } => {
            (StatusCode::CONFLICT, Envelope::err([err.to_string()]))
        }
        InvoiceError::Crm(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

pub async fn create_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let party_id = PartyId::new(body.party_id);
    let invoice = create_draft_from_new(&path, party_id, body.kind, body.lines).map_err(|e| {
        let (s, env) = map_invoice(e);
        (s, Json(env))
    })?;
    let journal_entry =
        journal_suggestion(&invoice, &actor, &InvoiceConfig::default()).map_err(|e| {
            let (s, env) = map_invoice(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "invoice": invoice,
        "journal_entry": journal_entry,
    }))))
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
    if let Some(raw_id) = query.invoice_id {
        let id = InvoiceId::new(raw_id);
        let invoice = get_invoice(&path, &id).map_err(|e| {
            let (s, env) = map_invoice(e);
            (s, Json(env))
        })?;
        let invoice = invoice.ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(Envelope::err([format!("invoice not found: {id}")])),
            )
        })?;
        return Ok(Json(Envelope::ok(serde_json::to_value(invoice).unwrap())));
    }
    let invoices = list_invoices(&path).map_err(|e| {
        let (s, env) = map_invoice(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(invoices).unwrap())))
}

#[cfg(test)]
mod http_tests {
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_plugin_crm::upsert_party;
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
    async fn invoice_create_and_list() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let party = upsert_party(&company_path, None, "Buyer ApS".into()).unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
        };
        let app = router(state);
        let (kind, id) = actor_headers(&owner);
        let create_body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "party_id": party.id.to_string(),
            "kind": "sale",
            "lines": [{
                "description": "Consulting",
                "amount_minor": 12500,
                "currency": "DKK",
            }],
        });
        let create_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/invoices/drafts")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::from(create_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_res.status(), StatusCode::OK);
        let create_bytes = axum::body::to_bytes(create_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_env: Envelope<Value> = serde_json::from_slice(&create_bytes).unwrap();
        let data = create_env.data.unwrap();
        let invoice_id = data["invoice"]["id"].as_str().unwrap().to_string();
        assert!(data["journal_entry"]["legs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l["party_id"].is_string()));
        let list_uri = format!(
            "/api/v1/invoices/drafts?company={}",
            company_path.to_string_lossy(),
        );
        let list_res = app
            .clone()
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
        let get_uri = format!(
            "/api/v1/invoices/drafts?company={}&invoice_id={}",
            company_path.to_string_lossy(),
            invoice_id,
        );
        let get_res = app
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
    }

    #[tokio::test]
    async fn invoice_actor_denied() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        upsert_party(&company_path, None, "Buyer".into()).unwrap();
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
            "party_id": "party_buyer",
            "kind": "sale",
            "lines": [{
                "description": "Blocked",
                "amount_minor": 100,
                "currency": "DKK",
            }],
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/invoices/drafts")
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
