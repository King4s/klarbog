//! Klarbog DEV UI — Askama SSR (ADR-018). No application JavaScript.

mod pages;

use crate::AppState;
use axum::response::Redirect;
use axum::routing::get;
use axum::Router;
use std::path::PathBuf;
use tower_http::services::ServeDir;

/// CSS-only assets (no JS). Default: repo `ui/` (styles.css).
pub fn ui_assets_dir() -> PathBuf {
    if let Ok(p) = std::env::var("KLARBOG_UI_DIR") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui")
}

pub fn mount_ui(router: Router<AppState>) -> Router<AppState> {
    let assets = ui_assets_dir();
    let static_css = ServeDir::new(assets);
    router
        .route("/", get(|| async { Redirect::temporary("/ui/") }))
        .route("/ui", get(|| async { Redirect::permanent("/ui/") }))
        .route("/ui/", get(pages::home))
        .route(
            "/ui/settings",
            get(pages::settings_get).post(pages::settings_post),
        )
        .route(
            "/ui/parties",
            get(pages::parties_get).post(pages::parties_post),
        )
        .route(
            "/ui/invoices",
            get(pages::invoices_get).post(pages::invoices_post),
        )
        .route("/ui/bank", get(pages::bank_get).post(pages::bank_post))
        .route("/ui/bilag", get(pages::bilag_get).post(pages::bilag_post))
        .route("/ui/bilag/attach", axum::routing::post(pages::bilag_attach))
        .route(
            "/ui/journal",
            get(pages::journal_get).post(pages::journal_post),
        )
        .route("/ui/chart", get(pages::chart_get))
        .nest_service("/ui/assets", static_css)
}

#[cfg(test)]
mod tests {
    use super::ui_assets_dir;
    use crate::{default_state, router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[test]
    fn ui_assets_include_styles() {
        let dir = ui_assets_dir();
        assert!(
            dir.join("styles.css").is_file(),
            "expected {}/styles.css",
            dir.display()
        );
    }

    #[tokio::test]
    async fn ui_home_is_rust_ssr() {
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
        assert!(
            html.contains("Askama") || html.contains("Rust SSR"),
            "UI should advertise Rust SSR"
        );
        assert!(
            !html.contains("/ui/app.js"),
            "SSR UI must not load the retired JS SPA"
        );
    }

    /// The invoice commit flow carries the journal entry as JSON through the
    /// form; the confirm token is digest-bound, so the serde round-trip must
    /// preserve `payload_digest` exactly or commit would fail closed.
    #[test]
    fn ui_entry_json_roundtrip_preserves_digest() {
        use klarbog_core::payload_digest;
        use klarbog_journal::{Direction, JournalEntry, Leg};
        use klarbog_types::{Actor, Currency, MinorAmount};

        let entry = JournalEntry {
            as_of: chrono::Utc::now(),
            memo: "invoice:inv_x:payment æøå".into(),
            actor: Actor::user("ui-dev"),
            legs: vec![
                Leg {
                    account: "5800".into(),
                    direction: Direction::Debit,
                    amount: MinorAmount::from_minor(12_500),
                    currency: Currency::new("DKK").unwrap(),
                    party_id: None,
                },
                Leg {
                    account: "1000".into(),
                    direction: Direction::Credit,
                    amount: MinorAmount::from_minor(12_500),
                    currency: Currency::new("DKK").unwrap(),
                    party_id: None,
                },
            ],
        };
        let company = std::path::Path::new("/tmp/co");
        let before = payload_digest(company, &entry).unwrap();
        let json = serde_json::to_string(&entry).unwrap();
        let back: JournalEntry = serde_json::from_str(&json).unwrap();
        let after = payload_digest(company, &back).unwrap();
        assert_eq!(before, after, "entry JSON round-trip must keep the digest");
    }
}
