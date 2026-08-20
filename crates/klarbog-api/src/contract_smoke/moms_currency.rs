//! Wave8: moms-suggest + multi-currency fail-closed (HTTP 400) in contract smoke.

use super::{actor_headers, json_envelope, json_req};
use crate::{router, AppState};
use axum::http::StatusCode;
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_types::Actor;
use std::sync::Arc;
use tempfile::tempdir;

/// Offline smoke: moms-suggest `#vat25` i64 legs + mixed-currency import preview → 400.
#[tokio::test]
async fn contract_smoke_moms_suggest_and_multi_currency_400() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let company = company_path.to_string_lossy().to_string();
    let (kind, id) = actor_headers(&owner);

    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);

    // moms-suggest: tax-inclusive gross → net+vat i64 legs; never auto-posts
    let (st, moms) = json_req(
        app.clone(),
        "POST",
        "/api/v1/journal/moms-suggest",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "gross_minor": 12_500_i64,
            "memo": "supplies #vat25"
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(moms["suggested"], true);
    assert_eq!(moms["gross_minor"], 12_500);
    assert_eq!(moms["net_minor"], 10_000);
    assert_eq!(moms["vat_minor"], 2_500);
    assert_eq!(moms["auto_post"], false);
    assert_eq!(moms["legs"][1]["amount_minor"], 2_500);

    // multi-currency fail-closed: mixed DKK+EUR CSV vs company DKK → 400
    let csv = "Completed Date,Description,Amount,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
    let (st, env) = json_envelope(
        app,
        "POST",
        "/api/v1/bank/import/preview",
        kind,
        &id,
        serde_json::json!({
            "company": company,
            "provider": "revolut",
            "source": "csv",
            "currency": "DKK",
            "csv": csv,
        }),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(!env.ok);
    assert!(env
        .errors
        .iter()
        .any(|e| e.contains("mixed currencies") || e.contains("currency")));
}
