//! Tests for wave2 bank MCP tools.

use super::{
    bank_reconcile_apply, bank_reconcile_suggest, bank_stripe_consume, revolut_oauth_refresh,
};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_bank::{
    ingest_stripe_webhook, save_revolut_tokens, sign_test_payload, RevolutStoredTokens,
    StripeWebhookConfig, STRIPE_WEBHOOKS_CONSUMED,
};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;

const PAYOUT_FIXTURE: &str =
    include_str!("../../../klarbog-plugin-bank/tests/fixtures/stripe_webhook_payout_paid.json");
const TEST_SECRET: &str = "whsec_test_fixture_secret";

#[tokio::test]
async fn mcp_stripe_consume_defaults_dry_run() {
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

    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = bank_stripe_consume(&args, dir.path()).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert_eq!(data["dry_run"], true);
    assert_eq!(data["consumed_count"], 1);
    assert!(!company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
}

#[tokio::test]
async fn mcp_stripe_consume_confirm_persists() {
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

    let args = json!({
        "company": company_path.to_string_lossy(),
        "confirm": true,
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = bank_stripe_consume(&args, dir.path()).await;
    assert!(env.ok);
    assert_eq!(env.data.unwrap()["dry_run"], false);
    assert!(company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
}

#[tokio::test]
async fn mcp_reconcile_suggest_with_parsed_rows() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &company_path,
        None,
        "Nordic Supply".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    create_draft_from_new(
        &company_path,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: 50_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "rows": [{
            "date": "2026-05-20",
            "text": "Customer payment Nordic Supply consulting",
            "amount_minor": 50000
        }],
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = bank_reconcile_suggest(&args, dir.path()).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert_eq!(data["count"], 1);
    assert_eq!(data["rows"][0]["suggestions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn mcp_reconcile_suggest_authz_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "rows": [{
            "date": "2026-05-20",
            "text": "x",
            "amount_minor": 100
        }],
        "actor_kind": "user",
        "actor_id": "intruder",
    });
    let env = bank_reconcile_suggest(&args, dir.path()).await;
    assert!(!env.ok);
    assert!(env.errors[0].contains("actor not in policy"));
}

#[tokio::test]
async fn mcp_reconcile_apply_returns_entry() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &company_path,
        None,
        "Nordic Supply".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &company_path,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: 50_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": inv.id.to_string(),
        "row": {
            "date": "2026-05-20",
            "text": "Customer payment Nordic Supply consulting",
            "amount_minor": 50000
        },
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = bank_reconcile_apply(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert!(data["entry"]["memo"].as_str().unwrap().contains("bank:"));
    assert!(data["entry"]["legs"].as_array().unwrap()[0]["party_id"].is_string());
    assert_eq!(data["forced"], false);
    assert!(data.get("confirm_token").is_none());
}

#[tokio::test]
async fn mcp_reconcile_apply_preview_issues_confirm_token() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &company_path,
        None,
        "Nordic Supply".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &company_path,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: 50_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "invoice_id": inv.id.to_string(),
        "preview": true,
        "row": {
            "date": "2026-05-20",
            "text": "Customer payment Nordic Supply consulting",
            "amount_minor": 50000
        },
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = bank_reconcile_apply(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert!(data["entry"]["memo"].as_str().unwrap().contains("bank:"));
    let token = data["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(data["payload_digest"].as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn mcp_revolut_refresh_fail_closed_no_tokens_in_body() {
    let _lock = super::super::ENV_TEST_LOCK.lock().await;
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    save_revolut_tokens(
        &company_path,
        &RevolutStoredTokens {
            access_token: "access-only-secret".into(),
            refresh_token: None,
            expires_at: Some(1),
            token_type: None,
        },
    )
    .unwrap();
    let _env = [
        EnvGuard::set("KLARBOG_REVOLUT_CLIENT_ID", "client-id"),
        EnvGuard::set("KLARBOG_REVOLUT_CLIENT_SECRET", "client-secret"),
        EnvGuard::set(
            "KLARBOG_REVOLUT_REDIRECT_URI",
            "https://example.test/callback",
        ),
        EnvGuard::set("KLARBOG_REVOLUT_TOKEN_URL", "https://example.test/token"),
    ];
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = revolut_oauth_refresh(&args, dir.path()).await;
    assert!(!env.ok);
    let body = serde_json::to_string(&env).unwrap();
    assert!(!body.contains("access-only-secret"));
    assert!(!body.contains("\"access_token\""));
    assert!(!body.contains("\"refresh_token\""));
    assert!(env.errors[0].contains("refresh_token") || env.errors[0].contains("Missing"));
}

#[tokio::test]
async fn mcp_wave2_actor_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "intruder",
    });
    let env = bank_stripe_consume(&args, dir.path()).await;
    assert!(!env.ok);
    assert!(env.errors[0].contains("actor not in policy"));
}

struct EnvGuard {
    key: &'static str,
    prev: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, val: &str) -> Self {
        let prev = std::env::var(key).ok();
        unsafe { std::env::set_var(key, val) };
        Self { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}
