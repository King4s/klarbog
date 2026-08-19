use super::*;
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

const FIXTURE: &str =
    "Dato;Tekst;Beløb\n19.08.2026;Office supplies;-125,50\n20.08.2026;Customer payment;500,00\n";

fn actor_headers(actor: &Actor) -> (&'static str, String) {
    let kind = match actor.kind {
        ActorKind::User => "user",
        ActorKind::Agent => "agent",
        ActorKind::System => "system",
    };
    (kind, actor.id.clone())
}

#[tokio::test]
async fn bank_preview_generic_dk_csv() {
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
        "provider": "generic_dk",
        "source": "csv",
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
    assert_eq!(data["source"], "csv");
    assert_eq!(data["drafts"][0]["amount_minor"], -12550);
    assert!(data["drafts"][0]["memo"]
        .as_str()
        .unwrap()
        .contains("Office supplies"));
}

#[tokio::test]
async fn bank_preview_revolut_api_missing_env() {
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
    let _guard = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "provider": "revolut",
        "source": "api",
        "currency": "DKK",
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
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
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
        "provider": "generic_dk",
        "source": "csv",
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

#[tokio::test]
async fn reconcile_suggest_with_parsed_rows() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(&company_path, None, "Acme ApS".into()).unwrap();
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
            "text": "Customer payment Acme ApS",
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

struct EnvGuard {
    key: &'static str,
    prev: Option<String>,
}

impl EnvGuard {
    fn unset(key: &'static str) -> Self {
        let prev = std::env::var(key).ok();
        unsafe { std::env::remove_var(key) };
        Self { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}
