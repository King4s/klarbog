//! Serve the local DEV web UI (ADR-012).

use axum::response::Redirect;
use axum::routing::get;
use axum::Router;
use std::path::PathBuf;
use tower_http::services::ServeDir;

/// Resolve UI asset directory: `KLARBOG_UI_DIR`, else repo `ui/` next to the workspace.
pub fn ui_dir() -> PathBuf {
    if let Ok(p) = std::env::var("KLARBOG_UI_DIR") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui")
}

pub fn mount_ui<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let dir = ui_dir();
    if !dir.is_dir() {
        tracing::warn!(
            path = %dir.display(),
            "Klarbog UI directory missing; /ui will 404 until ui/ is present or KLARBOG_UI_DIR is set"
        );
        return router.route("/", get(|| async { Redirect::temporary("/api/v1/status") }));
    }
    let static_files = ServeDir::new(dir).append_index_html_on_directories(true);
    router
        .route("/", get(|| async { Redirect::temporary("/ui/") }))
        .nest_service("/ui", static_files)
}

#[cfg(test)]
mod tests {
    use super::ui_dir;
    use crate::{default_state, router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[test]
    fn ui_dir_points_at_repo_ui() {
        let dir = ui_dir();
        assert!(
            dir.join("index.html").is_file(),
            "expected {}/index.html (run tests from workspace checkout)",
            dir.display()
        );
    }

    #[tokio::test]
    async fn ui_index_is_served() {
        let app = router(default_state());
        let res = app
            .oneshot(Request::builder().uri("/ui/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024)
            .await
            .unwrap();
        let html = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(html.contains("Klarbog"), "UI should brand Klarbog");
    }
}
