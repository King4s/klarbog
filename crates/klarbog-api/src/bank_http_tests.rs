use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_types::{Actor, ActorKind, Envelope};
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::Mutex;
use tower::ServiceExt;

static ENV_TEST_LOCK: Mutex<()> = Mutex::const_new(());

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
    let _lock = ENV_TEST_LOCK.lock().await;
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
    let _guards = [
        EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN"),
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_ID"),
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_SECRET"),
    ];
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
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = std::str::from_utf8(bytes.as_ref()).unwrap();
    assert!(!body_str.contains("\"access_token\""));
    assert!(!body_str.contains("\"refresh_token\""));
}

#[tokio::test]
async fn bank_preview_stripe_api_missing_env() {
    let _lock = ENV_TEST_LOCK.lock().await;
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
    let _guard = EnvGuard::unset("KLARBOG_STRIPE_SECRET_KEY");
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "provider": "stripe",
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
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = std::str::from_utf8(bytes.as_ref()).unwrap();
    assert!(!body_str.contains("\"secret_key\""));
    assert!(!body_str.contains("sk_"));
}

#[tokio::test]
async fn bank_preview_revolut_csv_mixed_currency_vs_dkk() {
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
    let csv = "Completed Date,Description,Amount,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "provider": "revolut",
        "source": "csv",
        "currency": "DKK",
        "csv": csv,
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(env.errors.iter().any(|e| e.contains("mixed currencies")));
}

#[tokio::test]
async fn bank_preview_stripe_csv_eur_vs_company_dkk() {
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
    let csv = "Created,Description,Net,Currency\n2026-01-01,A,-10.00,EUR\n";
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "provider": "stripe",
        "source": "csv",
        "currency": "DKK",
        "csv": csv,
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(env
        .errors
        .iter()
        .any(|e| e.contains("currency mismatch") && e.contains("DKK")));
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
