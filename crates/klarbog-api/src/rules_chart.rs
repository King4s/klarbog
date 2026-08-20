//! GET /api/v1/rules/chart — read-only DEV chart stub (codes + labels).

use crate::actor::parse_actor;
use crate::invoice::{authorize_company, map_core};
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_rules_dk::{chart_stub_entries, RULE_KNOWN_ACCOUNT};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct ChartQuery {
    pub company: String,
}

/// Read-only stub chart list — AuthZ/allowlist like other read tools; no journal write.
pub async fn rules_chart(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ChartQuery>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(&query.company);
    authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (status, env) = map_core(e);
            (status, Json(env))
        })?;

    Ok(Json(Envelope::ok(json!({
        "stub": true,
        "accounts": chart_stub_entries(),
        "rule_known_account": RULE_KNOWN_ACCOUNT,
    }))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_types::Actor;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tower::ServiceExt;

    async fn app_with_company() -> (axum::Router, tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = dir.path().join("co");
        init_company(&company, "Demo", &owner).await.unwrap();
        let state = AppState {
            confirm: Arc::new(ConfirmStore::default()),
            allowlist_root: dir.path().to_path_buf(),
            registry: Arc::new(default_registry()),
        };
        (router(state), dir, company)
    }

    #[tokio::test]
    async fn rules_chart_returns_stub_codes() {
        let (app, _dir, company) = app_with_company().await;
        let uri = format!("/api/v1/rules/chart?company={}", company.to_string_lossy());
        let res = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&uri)
                    .header("x-klarbog-actor-kind", "user")
                    .header("x-klarbog-actor-id", "owner")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["stub"], true);
        assert_eq!(data["rule_known_account"], RULE_KNOWN_ACCOUNT);
        let accounts = data["accounts"].as_array().unwrap();
        assert_eq!(accounts.len(), 4);
        assert_eq!(accounts[0]["code"], "1000");
        assert_eq!(accounts[0]["label"], "Bank");
        assert_eq!(accounts[3]["code"], "4000-6999");
        assert_eq!(accounts[3]["min"], 4000);
        assert_eq!(accounts[3]["max"], 6999);
    }

    #[tokio::test]
    async fn rules_chart_outside_allowlist_is_denied() {
        let (app, _dir, _company) = app_with_company().await;
        let res = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/rules/chart?company=/tmp/not-allowed-co")
                    .header("x-klarbog-actor-kind", "user")
                    .header("x-klarbog-actor-id", "owner")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            res.status() == StatusCode::BAD_REQUEST || res.status() == StatusCode::FORBIDDEN,
            "status={}",
            res.status()
        );
    }
}
