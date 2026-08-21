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
        .route("/ui/chart", get(pages::chart_get).post(pages::chart_post))
        .nest_service("/ui/assets", static_css)
}

#[cfg(test)]
mod tests;
