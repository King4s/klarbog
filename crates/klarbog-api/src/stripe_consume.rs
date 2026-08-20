//! POST /api/v1/bank/stripe/consume — queue → bank draft rows (ADR-009). No journal post.

use crate::actor::parse_actor;
use crate::bank::{authorize_company, map_core};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_bank::{consume_stripe_webhook_queue, ConsumeOpts, StripeWebhookError};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct ConsumeBody {
    pub company: String,
    /// Default true (fail-closed preview). Set false only with `confirm: true`.
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    /// Fail-closed apply flag (mirrors retention purge). Overrides dry_run when true.
    #[serde(default)]
    pub confirm: bool,
    pub limit: Option<usize>,
}

fn default_dry_run() -> bool {
    true
}

fn map_webhook(err: StripeWebhookError) -> (StatusCode, Envelope<Value>) {
    let status = match &err {
        StripeWebhookError::InvalidJson(_) | StripeWebhookError::MissingField(_) => {
            StatusCode::BAD_REQUEST
        }
        StripeWebhookError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Envelope::err([err.to_string()]))
}

pub async fn consume(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ConsumeBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    // Fail-closed like retention purge: only confirm:true persists.
    let dry_run = !body.confirm;
    let _ = body.dry_run; // accepted for API clarity; confirm wins
    let report = consume_stripe_webhook_queue(
        &path,
        ConsumeOpts {
            dry_run,
            limit: body.limit,
        },
    )
    .map_err(|e| {
        let (s, env) = map_webhook(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(report).unwrap())))
}

#[cfg(test)]
mod http_tests {
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_plugin_bank::{
        ingest_stripe_webhook, sign_test_payload, StripeWebhookConfig, STRIPE_WEBHOOKS_CONSUMED,
    };
    use klarbog_types::{Actor, Envelope};
    use serde_json::Value;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tower::ServiceExt;

    const PAYOUT_FIXTURE: &str =
        include_str!("../../klarbog-plugin-bank/tests/fixtures/stripe_webhook_payout_paid.json");
    const TEST_SECRET: &str = "whsec_test_fixture_secret";

    fn actor_headers() -> [(&'static str, &'static str); 2] {
        [
            ("x-klarbog-actor-kind", "user"),
            ("x-klarbog-actor-id", "owner"),
        ]
    }

    #[tokio::test]
    async fn consume_defaults_to_dry_run() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let cfg = StripeWebhookConfig {
            webhook_secret: TEST_SECRET.into(),
        };
        let ts = chrono::Utc::now().timestamp();
        let sig = sign_test_payload(PAYOUT_FIXTURE.as_bytes(), TEST_SECRET, ts);
        ingest_stripe_webhook(&company_path, PAYOUT_FIXTURE.as_bytes(), Some(&sig), &cfg).unwrap();

        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
            api_token: None,
            session_secret: None,
            session_cookie_secure: false,
        };
        let app = router(state);
        let body = serde_json::json!({ "company": company_path.to_string_lossy() });
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/v1/bank/stripe/consume")
            .header("content-type", "application/json");
        for (k, v) in actor_headers() {
            req = req.header(k, v);
        }
        let res = app
            .oneshot(req.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
        let data = env.data.unwrap();
        assert_eq!(data["dry_run"], true);
        assert_eq!(data["consumed_count"], 1);
        assert_eq!(data["rows"][0]["amount_minor"], -100_000);
        assert!(!company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
    }

    #[tokio::test]
    async fn confirm_persists_consumed_sidecar() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let cfg = StripeWebhookConfig {
            webhook_secret: TEST_SECRET.into(),
        };
        let ts = chrono::Utc::now().timestamp();
        let sig = sign_test_payload(PAYOUT_FIXTURE.as_bytes(), TEST_SECRET, ts);
        ingest_stripe_webhook(&company_path, PAYOUT_FIXTURE.as_bytes(), Some(&sig), &cfg).unwrap();

        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
            api_token: None,
            session_secret: None,
            session_cookie_secure: false,
        };
        let app = router(state);
        let body = serde_json::json!({
            "company": company_path.to_string_lossy(),
            "confirm": true
        });
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/v1/bank/stripe/consume")
            .header("content-type", "application/json");
        for (k, v) in actor_headers() {
            req = req.header(k, v);
        }
        let res = app
            .oneshot(req.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(env.data.unwrap()["dry_run"], false);
        assert!(company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
    }

    #[tokio::test]
    async fn missing_actor_is_400() {
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
        let body = serde_json::json!({ "company": company_path.to_string_lossy() });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/bank/stripe/consume")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
