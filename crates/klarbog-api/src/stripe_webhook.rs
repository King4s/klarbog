//! Stripe webhook ingress (ADR-009). Verify signature, queue JSON, draft rows — no post.

use crate::AppState;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_bank::{ingest_stripe_webhook, StripeWebhookConfig, StripeWebhookError};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct WebhookQuery {
    pub company: String,
}

fn map_webhook(err: StripeWebhookError) -> (StatusCode, Envelope<Value>) {
    let status = match &err {
        StripeWebhookError::MissingSignature
        | StripeWebhookError::InvalidSignatureHeader
        | StripeWebhookError::SignatureMismatch
        | StripeWebhookError::TimestampSkew => StatusCode::BAD_REQUEST,
        StripeWebhookError::InvalidJson(_)
        | StripeWebhookError::MissingField(_)
        | StripeWebhookError::UnsupportedEvent(_) => StatusCode::BAD_REQUEST,
        StripeWebhookError::Config(_) => StatusCode::SERVICE_UNAVAILABLE,
        StripeWebhookError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Envelope::err([err.to_string()]))
}

fn map_core(err: CoreError) -> (StatusCode, Envelope<Value>) {
    match err {
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

async fn resolve_company(allowlist_root: &Path, company: &Path) -> Result<PathBuf, CoreError> {
    let path = assert_company_path(allowlist_root, company)?;
    open_existing(&path).await?;
    Ok(path)
}

pub async fn handle(
    State(state): State<AppState>,
    Query(query): Query<WebhookQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let cfg = StripeWebhookConfig::from_env().map_err(|e| {
        let (s, env) = map_webhook(StripeWebhookError::Config(e));
        (s, Json(env))
    })?;
    let company = PathBuf::from(query.company);
    let path = resolve_company(&state.allowlist_root, &company)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok());
    let entry = ingest_stripe_webhook(&path, &body, sig, &cfg).map_err(|e| {
        let (s, env) = map_webhook(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "event_id": entry.event_id,
        "event_type": entry.event_type,
        "object_id": entry.object_id,
        "queued": true,
        "queue": klarbog_plugin_bank::STRIPE_WEBHOOKS_QUEUE,
        "draft": entry.draft,
    }))))
}

#[cfg(test)]
mod http_tests {
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_plugin_bank::sign_test_payload;
    use klarbog_types::{Actor, Envelope};
    use serde_json::Value;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tower::ServiceExt;

    const PAYOUT_FIXTURE: &str =
        include_str!("../../klarbog-plugin-bank/tests/fixtures/stripe_webhook_payout_paid.json");
    const TEST_SECRET: &str = "whsec_test_fixture_secret";

    #[tokio::test]
    async fn stripe_webhook_accepts_signed_fixture() {
        let _lock = crate::ENV_TEST_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
            api_token: None,
        };
        let app = router(state);
        let ts = chrono::Utc::now().timestamp();
        let sig = sign_test_payload(PAYOUT_FIXTURE.as_bytes(), TEST_SECRET, ts);
        let uri = format!(
            "/api/v1/webhooks/stripe?company={}",
            company_path.to_string_lossy()
        );
        let _guard = EnvGuard::set("KLARBOG_STRIPE_WEBHOOK_SECRET", TEST_SECRET);
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&uri)
                    .header("content-type", "application/json")
                    .header("stripe-signature", sig)
                    .body(Body::from(PAYOUT_FIXTURE))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK);
        let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
        let data = env.data.unwrap();
        assert_eq!(data["event_type"], "payout.paid");
        assert_eq!(data["queued"], true);
        assert!(data["draft"]["amount_minor"].as_i64().unwrap() < 0);
        let queue = company_path.join("stripe_webhooks/queue.jsonl");
        assert!(queue.exists());
    }

    #[tokio::test]
    async fn stripe_webhook_missing_secret_is_503() {
        let _lock = crate::ENV_TEST_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
            api_token: None,
        };
        let app = router(state);
        let uri = format!(
            "/api/v1/webhooks/stripe?company={}",
            company_path.to_string_lossy()
        );
        let _guard = EnvGuard::unset("KLARBOG_STRIPE_WEBHOOK_SECRET");
        let status = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&uri)
                    .header("stripe-signature", "t=1,v1=deadbeef")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prev = std::env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self { key, prev }
        }

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
}
