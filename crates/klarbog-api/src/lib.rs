//! DEV-only HTTP surface. Default bind is loopback in the binary (ADR-003 / ADR-014).

/// Serializes process-env mutation across Revolut/Stripe/OAuth tests (parallel VERIFY).
#[cfg(test)]
pub(crate) static ENV_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

mod actor;
mod auth_session;
mod auth_token;
mod bank;
mod bank_reconcile;
mod bank_reconcile_apply;
mod crm;
mod documents;
mod documents_exceptions;
mod invoice;
mod invoice_lifecycle;
mod journal;
mod moms_suggest;
mod retention;
mod retention_erase;
mod revolut_oauth;
mod rules_chart;
mod stripe_consume;
mod stripe_reconcile_apply_preview;
mod stripe_reconcile_suggest;
mod stripe_webhook;
mod ui;

#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod auth_session_tests;
#[cfg(test)]
mod contract_smoke;
#[cfg(test)]
mod documents_tests;
#[cfg(test)]
mod invoice_lifecycle_http_tests;
#[cfg(test)]
mod invoice_lifecycle_preview_http_tests;
#[cfg(test)]
mod retention_erase_http_tests;
#[cfg(test)]
mod retention_tests;
#[cfg(test)]
mod revolut_oauth_http_tests;
#[cfg(test)]
mod stripe_reconcile_apply_preview_tests;
#[cfg(test)]
mod stripe_reconcile_suggest_tests;

use axum::extract::State;
use axum::middleware;
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
    /// When `Some` and non-empty, `/api/v1/*` requires bearer (ADR-016).
    pub api_token: Option<Arc<str>>,
    /// When set with `api_token`, enables login cookie (ADR-017).
    pub session_secret: Option<Arc<str>>,
    /// `Secure` on `klarbog_session` (non-loopback / env flag).
    pub session_cookie_secure: bool,
}

impl AppState {
    pub fn session_enabled(&self) -> bool {
        self.api_token.as_deref().is_some_and(|t| !t.is_empty())
            && self
                .session_secret
                .as_deref()
                .is_some_and(|t| !t.is_empty())
    }
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
    let mut api = Router::new()
        .route("/health", get(health))
        .route("/api/v1/status", get(status))
        .route("/api/v1/journal/preview", post(journal::preview))
        .route("/api/v1/journal/commit", post(journal::commit))
        .route(
            "/api/v1/journal/moms-suggest",
            post(moms_suggest::moms_suggest),
        )
        .route("/api/v1/rules/chart", get(rules_chart::rules_chart))
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
        .route(
            "/api/v1/invoices/mark-part-paid",
            post(invoice_lifecycle::mark_part_paid),
        )
        .route("/api/v1/documents", post(documents::attach))
        .route("/api/v1/documents", get(documents::list_docs))
        .route("/api/v1/documents", delete(documents::delete_doc))
        .route("/api/v1/exceptions", post(documents_exceptions::raise))
        .route("/api/v1/exceptions", get(documents_exceptions::list_exc))
        .route("/api/v1/exceptions", patch(documents_exceptions::close_exc))
        .route("/api/v1/retention", get(retention::get_retention))
        .route("/api/v1/backup", post(retention::post_backup))
        .route("/api/v1/gdpr-export", post(retention::post_gdpr_export))
        .route(
            "/api/v1/gdpr/erase-party",
            post(retention_erase::post_erase_party),
        )
        .route("/api/v1/retention/purge", post(retention::post_purge))
        .route("/api/v1/bank/import/preview", post(bank::preview))
        .route(
            "/api/v1/bank/reconcile/suggest",
            post(bank_reconcile::reconcile_suggest),
        )
        .route(
            "/api/v1/bank/reconcile/apply",
            post(bank_reconcile_apply::reconcile_apply),
        )
        .route("/api/v1/webhooks/stripe", post(stripe_webhook::handle))
        .route("/api/v1/bank/stripe/consume", post(stripe_consume::consume))
        .route(
            "/api/v1/bank/stripe/reconcile-suggest",
            post(stripe_reconcile_suggest::reconcile_suggest),
        )
        .route(
            "/api/v1/bank/stripe/reconcile-apply-preview",
            post(stripe_reconcile_apply_preview::reconcile_apply_preview),
        )
        .route(
            "/api/v1/revolut/oauth/start",
            get(revolut_oauth::oauth_start_handler),
        )
        .route(
            "/api/v1/revolut/oauth/callback",
            post(revolut_oauth::oauth_callback_handler),
        )
        .route(
            "/api/v1/revolut/oauth/refresh",
            post(revolut_oauth::oauth_refresh_handler),
        );

    if state.session_enabled() {
        api = api
            .route("/api/v1/auth/login", post(auth_session::login))
            .route("/api/v1/auth/logout", post(auth_session::logout));
    }

    let state_for_mw = state.clone();
    ui::mount_ui(api)
        .layer(middleware::from_fn_with_state(
            state_for_mw,
            auth_token::api_token_middleware,
        ))
        .with_state(state)
}

pub fn default_allowlist_root() -> PathBuf {
    std::env::var("KLARBOG_ALLOWLIST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn api_token_from_env() -> Option<Arc<str>> {
    match std::env::var("KLARBOG_API_TOKEN") {
        Ok(v) => {
            let t = v.trim();
            if t.is_empty() {
                None
            } else {
                Some(Arc::<str>::from(t))
            }
        }
        Err(_) => None,
    }
}

fn session_secret_from_env() -> Option<Arc<str>> {
    match std::env::var("KLARBOG_SESSION_SECRET") {
        Ok(v) => {
            let t = v.trim();
            if t.is_empty() {
                None
            } else {
                Some(Arc::<str>::from(t))
            }
        }
        Err(_) => None,
    }
}

fn env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let lower = v.trim().to_ascii_lowercase();
            lower == "1" || lower == "true" || lower == "yes"
        }
        Err(_) => false,
    }
}

/// Secure cookie when explicitly flagged, or when non-loopback bind is allowed
/// with an explicit non-loopback `KLARBOG_BIND` (ADR-017).
fn session_cookie_secure_from_env() -> bool {
    if env_truthy("KLARBOG_SESSION_COOKIE_SECURE") {
        return true;
    }
    if !env_truthy("KLARBOG_ALLOW_NON_LOOPBACK") {
        return false;
    }
    match std::env::var("KLARBOG_BIND") {
        Ok(raw) => {
            let raw = raw.trim();
            if raw.is_empty() {
                return false;
            }
            raw.parse::<std::net::SocketAddr>()
                .map(|a| !a.ip().is_loopback())
                .unwrap_or(false)
        }
        Err(_) => false,
    }
}

pub fn default_state() -> AppState {
    AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: default_allowlist_root(),
        registry: Arc::new(default_registry()),
        api_token: api_token_from_env(),
        session_secret: session_secret_from_env(),
        session_cookie_secure: session_cookie_secure_from_env(),
    }
}
