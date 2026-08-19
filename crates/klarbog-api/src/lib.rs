//! DEV-only HTTP surface. Bind loopback in the binary (ADR-003).

mod actor;
mod bank;
mod bank_reconcile;
mod crm;
mod documents;
mod invoice;
mod invoice_lifecycle;
mod journal;
mod retention;
mod revolut_oauth;
mod stripe_webhook;

#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod documents_tests;
#[cfg(test)]
mod retention_tests;

use axum::extract::State;
use axum::{routing::delete, routing::get, routing::patch, routing::post, Json, Router};
use klarbog_core::{default_registry, ConfirmStore};
use klarbog_plugin::{Capability, Registry};
use klarbog_types::Envelope;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub confirm: Arc<ConfirmStore>,
    pub allowlist_root: PathBuf,
    pub registry: Arc<Registry>,
}

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

async fn status(State(state): State<AppState>) -> Json<Envelope<Value>> {
    let cap_name = |c: Capability| match c {
        Capability::Read => "read",
        Capability::CrmWrite => "crm_write",
        Capability::JournalWrite => "journal_write",
        Capability::RulesValidate => "rules_validate",
    };
    let plugins: Vec<_> = state
        .registry
        .list()
        .map(|p| {
            serde_json::json!({
                "id": p.id(),
                "version": p.version(),
                "capabilities": p.capabilities().iter().map(|c| cap_name(*c)).collect::<Vec<_>>(),
            })
        })
        .collect();
    Json(Envelope::ok(serde_json::json!({
        "mode": "dev",
        "bind": "127.0.0.1:3195",
        "allowlist_root": state.allowlist_root,
        "plugins": plugins,
    })))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/status", get(status))
        .route("/api/v1/journal/preview", post(journal::preview))
        .route("/api/v1/journal/commit", post(journal::commit))
        .route("/api/v1/crm/parties", post(crm::upsert))
        .route("/api/v1/crm/parties", get(crm::list))
        .route("/api/v1/invoices/drafts", post(invoice::create_draft))
        .route("/api/v1/invoices/drafts", get(invoice::list))
        .route(
            "/api/v1/invoices/status",
            patch(invoice_lifecycle::patch_status_handler),
        )
        .route(
            "/api/v1/invoices/mark-paid",
            post(invoice_lifecycle::mark_paid),
        )
        .route("/api/v1/documents", post(documents::attach))
        .route("/api/v1/documents", get(documents::list_docs))
        .route("/api/v1/documents", delete(documents::delete_doc))
        .route("/api/v1/exceptions", post(documents::raise))
        .route("/api/v1/exceptions", get(documents::list_exc))
        .route("/api/v1/exceptions", patch(documents::close_exc))
        .route("/api/v1/retention", get(retention::get_retention))
        .route("/api/v1/backup", post(retention::post_backup))
        .route("/api/v1/gdpr-export", post(retention::post_gdpr_export))
        .route("/api/v1/retention/purge", post(retention::post_purge))
        .route("/api/v1/bank/import/preview", post(bank::preview))
        .route(
            "/api/v1/bank/reconcile/suggest",
            post(bank_reconcile::reconcile_suggest),
        )
        .route("/api/v1/webhooks/stripe", post(stripe_webhook::handle))
        .route(
            "/api/v1/revolut/oauth/start",
            get(revolut_oauth::oauth_start_handler),
        )
        .route(
            "/api/v1/revolut/oauth/callback",
            post(revolut_oauth::oauth_callback_handler),
        )
        .with_state(state)
}

pub fn default_allowlist_root() -> PathBuf {
    std::env::var("KLARBOG_ALLOWLIST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn default_state() -> AppState {
    AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: default_allowlist_root(),
        registry: Arc::new(default_registry()),
    }
}
