//! DEV-only HTTP surface. Bind loopback in the binary (ADR-003).

use axum::http::StatusCode;
use axum::{routing::get, routing::post, Json, Router};
use klarbog_types::Envelope;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct Health {
    pub ok: bool,
    pub service: &'static str,
    pub version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        service: "klarbog-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn status_stub() -> Json<Envelope<Value>> {
    Json(Envelope::ok(serde_json::json!({
        "mode": "dev",
        "bind": "127.0.0.1:3195"
    })))
}

async fn journal_stub() -> (StatusCode, Json<Envelope<Value>>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(Envelope::err(["journal write arrives in slice 2"])),
    )
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/status", get(status_stub))
        .route("/api/v1/journal", post(journal_stub))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_ok() {
        let app = router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn journal_is_stub() {
        let app = router();
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/journal")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
