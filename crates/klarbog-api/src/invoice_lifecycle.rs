//! Invoice lifecycle HTTP: PATCH status, POST mark-paid preview.

use crate::actor::parse_actor;
use crate::invoice::{authorize_company, map_core, map_invoice};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_invoice::{
    mark_paid_preview, patch_status, InvoiceConfig, InvoiceId, InvoiceStatus,
};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct PatchStatusBody {
    pub company: String,
    pub invoice_id: String,
    pub status: InvoiceStatus,
}

#[derive(Deserialize)]
pub struct MarkPaidBody {
    pub company: String,
    pub invoice_id: String,
}

pub async fn patch_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PatchStatusBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = InvoiceId::new(body.invoice_id);
    let invoice = patch_status(&path, &id, body.status).map_err(|e| {
        let (s, env) = map_invoice(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(invoice).unwrap())))
}

pub async fn mark_paid(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MarkPaidBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = InvoiceId::new(body.invoice_id);
    let (invoice, journal_entry) = mark_paid_preview(&path, &id, &actor, &InvoiceConfig::default())
        .map_err(|e| {
            let (s, env) = map_invoice(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "invoice": invoice,
        "journal_entry": journal_entry,
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

    async fn seed_invoice(company_path: &std::path::Path) -> (Actor, String) {
        let owner = Actor::user("owner");
        init_company(company_path, "Demo", &owner).await.unwrap();
        let party = upsert_party(company_path, None, "Buyer ApS".into()).unwrap();
        let invoice = create_draft_from_new(
            company_path,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Consulting".into(),
                amount_minor: 5000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        (owner, invoice.id.to_string())
    }

    #[tokio::test]
    async fn patch_status_and_mark_paid() {
        let dir = tempdir().unwrap();
        let company_path = dir.path().join("co");
        std::fs::create_dir_all(&company_path).unwrap();
        let (owner, invoice_id) = seed_invoice(&company_path).await;
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
        };
        let app = router(state);
        let (kind, id) = actor_headers(&owner);
        let patch_body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "invoice_id": invoice_id,
            "status": "sent",
        });
        let patch_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/v1/invoices/status")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::from(patch_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(patch_res.status(), StatusCode::OK);
        let paid_body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "invoice_id": invoice_id,
        });
        let paid_res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/invoices/mark-paid")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", kind)
                    .header("x-klarbog-actor-id", &id)
                    .body(Body::from(paid_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(paid_res.status(), StatusCode::OK);
        let paid_bytes = axum::body::to_bytes(paid_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let paid_env: Envelope<Value> = serde_json::from_slice(&paid_bytes).unwrap();
        let data = paid_env.data.unwrap();
        assert_eq!(data["invoice"]["status"], "paid");
        assert!(data["journal_entry"]["legs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l["party_id"].is_string()));
    }
}
