//! POST /api/v1/journal/moms-suggest — preview-only net+vat legs from `#vat25`.

use crate::actor::parse_actor;
use crate::invoice::{authorize_company, map_core};
use crate::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_plugin_rules_dk::{moms_post_suggestion, MomsPostSuggestionError};
use klarbog_types::Envelope;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct MomsSuggestBody {
    pub company: String,
    /// Tax-inclusive gross (moms-inkl.) in minor units.
    pub gross_minor: i64,
    /// Memo text; suggestion only when `#vat25` / `moms:25` / `#moms25` present.
    pub memo: String,
}

fn map_moms(err: MomsPostSuggestionError) -> (StatusCode, Envelope<Value>) {
    (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
}

/// Optional moms post **suggestion** — never posts; no ConfirmStore token.
pub async fn moms_suggest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MomsSuggestBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(&body.company);
    authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (status, env) = map_core(e);
            (status, Json(env))
        })?;

    match moms_post_suggestion(body.gross_minor, &body.memo) {
        Ok(Some(s)) => Ok(Json(Envelope::ok(json!({
            "suggested": true,
            "gross_minor": s.gross_minor,
            "net_minor": s.net_minor,
            "vat_minor": s.vat_minor,
            "rate_bps": s.rate_bps,
            "legs": s.legs,
            "auto_post": false,
        })))),
        Ok(None) => Ok(Json(Envelope::ok(json!({
            "suggested": false,
            "auto_post": false,
            "reason": "memo has no #vat25 / moms:25 / #moms25 tag",
        })))),
        Err(e) => {
            let (status, env) = map_moms(e);
            Err((status, Json(env)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_types::Actor;
    use serde_json::{json, Value};
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
            api_token: None,
            session_secret: None,
            session_cookie_secure: false,
        };
        (router(state), dir, company)
    }

    #[tokio::test]
    async fn moms_suggest_vat25_returns_legs() {
        let (app, _dir, company) = app_with_company().await;
        let body = json!({
            "company": company.to_string_lossy(),
            "gross_minor": 12500,
            "memo": "supplies #vat25",
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/journal/moms-suggest")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", "user")
                    .header("x-klarbog-actor-id", "owner")
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
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["suggested"], true);
        assert_eq!(data["net_minor"], 10_000);
        assert_eq!(data["vat_minor"], 2_500);
        assert_eq!(data["auto_post"], false);
        assert_eq!(data["legs"][0]["role"], "net");
        assert_eq!(data["legs"][1]["amount_minor"], 2_500);
    }

    #[tokio::test]
    async fn moms_suggest_without_tag_is_optional_none() {
        let (app, _dir, company) = app_with_company().await;
        let body = json!({
            "company": company.to_string_lossy(),
            "gross_minor": 12500,
            "memo": "no vat tag",
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/journal/moms-suggest")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", "user")
                    .header("x-klarbog-actor-id", "owner")
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
        assert_eq!(env.data.unwrap()["suggested"], false);
    }

    /// Wave15: negative gross is fail-closed HTTP 400 (i64 path; never suggests).
    #[tokio::test]
    async fn moms_suggest_negative_gross_is_400() {
        let (app, _dir, company) = app_with_company().await;
        let body = json!({
            "company": company.to_string_lossy(),
            "gross_minor": -1,
            "memo": "supplies #vat25",
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/journal/moms-suggest")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", "user")
                    .header("x-klarbog-actor-id", "owner")
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
        assert!(!env.ok);
        assert!(env.data.is_none());
    }

    /// Wave16: unsupported memo VAT rate is fail-closed HTTP 400 (never suggests).
    #[tokio::test]
    async fn moms_suggest_unsupported_memo_rate_is_400() {
        let (app, _dir, company) = app_with_company().await;
        let body = json!({
            "company": company.to_string_lossy(),
            "gross_minor": 12_500_i64,
            "memo": "supplies vat:12",
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/journal/moms-suggest")
                    .header("content-type", "application/json")
                    .header("x-klarbog-actor-kind", "user")
                    .header("x-klarbog-actor-id", "owner")
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
        assert!(!env.ok);
        assert!(env.data.is_none());
    }
}
