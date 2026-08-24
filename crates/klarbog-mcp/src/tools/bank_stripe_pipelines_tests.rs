//! Tests for Stripe reconcile pipeline MCP tools.

use super::{bank_stripe_reconcile_apply_preview, bank_stripe_reconcile_suggest};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_bank::{
    ingest_stripe_webhook, sign_test_payload, StripeWebhookConfig, STRIPE_WEBHOOKS_CONSUMED,
};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;

const CHARGE_FIXTURE: &str = include_str!(
    "../../../klarbog-plugin-bank/tests/fixtures/stripe_webhook_charge_succeeded.json"
);
const TEST_SECRET: &str = "whsec_test_fixture_secret";

async fn company_with_charge(match_amount: bool) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &company_path,
        None,
        "Customer payment".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();
    let amount = if match_amount { 24_275 } else { 99_999 };
    create_draft_from_new(
        &company_path,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widgets".into(),
            amount_minor: amount,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let cfg = StripeWebhookConfig {
        webhook_secret: TEST_SECRET.into(),
    };
    let ts = chrono::Utc::now().timestamp();
    let sig = sign_test_payload(CHARGE_FIXTURE.as_bytes(), TEST_SECRET, ts);
    ingest_stripe_webhook(&company_path, CHARGE_FIXTURE.as_bytes(), Some(&sig), &cfg).unwrap();
    (dir, company_path)
}

#[tokio::test]
async fn mcp_stripe_reconcile_suggest_defaults_dry_run() {
    let (dir, company_path) = company_with_charge(true).await;
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = bank_stripe_reconcile_suggest(&args, dir.path()).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert_eq!(data["consume"]["dry_run"], true);
    assert_eq!(data["consume"]["consumed_count"], 1);
    assert_eq!(data["matches"][0]["amount_minor"], 24_275);
    assert!(!data["matches"][0]["suggestions"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
}

#[tokio::test]
async fn mcp_stripe_reconcile_suggest_confirm_persists() {
    let (dir, company_path) = company_with_charge(true).await;
    let args = json!({
        "company": company_path.to_string_lossy(),
        "confirm_consume": true,
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = bank_stripe_reconcile_suggest(&args, dir.path()).await;
    assert!(env.ok);
    assert_eq!(env.data.unwrap()["consume"]["dry_run"], false);
    assert!(company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
}

#[tokio::test]
async fn mcp_stripe_reconcile_apply_preview_unique_safe() {
    let (dir, company_path) = company_with_charge(true).await;
    let store = ConfirmStore::default();
    let registry = default_registry();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = bank_stripe_reconcile_apply_preview(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.as_ref().unwrap();
    assert_eq!(data["consume"]["dry_run"], true);
    assert!(data["applied"].is_object());
    let token = data["applied"]["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["applied"]["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(
        data["applied"]["payload_digest"].as_str().unwrap().len(),
        64
    );
    assert!(data["applied"]["entry"]["memo"]
        .as_str()
        .unwrap()
        .contains("bank:"));
    assert!(!company_path.join(STRIPE_WEBHOOKS_CONSUMED).exists());
    let body = serde_json::to_string(&env).unwrap();
    assert!(!body.contains(TEST_SECRET));
    assert!(!body.contains("whsec_"));
}

#[tokio::test]
async fn mcp_stripe_reconcile_apply_preview_no_safe_match() {
    let (dir, company_path) = company_with_charge(false).await;
    let store = ConfirmStore::default();
    let registry = default_registry();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = bank_stripe_reconcile_apply_preview(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert!(data["applied"].is_null());
    assert_eq!(data["matches"][0]["amount_minor"], 24_275);
    assert!(data.get("confirm_token").is_none());
}

#[tokio::test]
async fn mcp_stripe_pipelines_actor_denied() {
    let (dir, company_path) = company_with_charge(true).await;
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "intruder",
    });
    let env = bank_stripe_reconcile_suggest(&args, dir.path()).await;
    assert!(!env.ok);
    assert!(env.errors[0].contains("actor not in policy"));
}
