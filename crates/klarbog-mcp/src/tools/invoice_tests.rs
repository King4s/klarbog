//! MCP tests: invoice create/list/patch + AuthZ fail-closed.

use super::*;
use klarbog_core::init_company;
use klarbog_plugin_crm::upsert_party;
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn mcp_invoice_create_list_patch_status() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();

    let create = invoice_create_draft(
        &json!({
            "company": co.to_string_lossy(),
            "party_id": party.id.to_string(),
            "kind": "sale",
            "lines": [{
                "description": "Item",
                "amount_minor": 12_500,
                "currency": "DKK"
            }],
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(create.ok, "{create:?}");
    let data = create.data.unwrap();
    assert_eq!(data["invoice"]["status"], "draft");
    assert_eq!(data["invoice"]["lines"][0]["amount_minor"], 12_500);
    assert!(data["journal_entry"]["legs"].as_array().unwrap().len() >= 2);
    let invoice_id = data["invoice"]["id"].as_str().unwrap().to_string();

    let listed = invoice_list(
        &json!({
            "company": co.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(listed.ok, "{listed:?}");
    assert_eq!(listed.data.unwrap().as_array().unwrap().len(), 1);

    let one = invoice_list(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": invoice_id,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(one.ok, "{one:?}");
    assert_eq!(one.data.unwrap()["id"], invoice_id);

    let patched = invoice_patch_status(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": invoice_id,
            "status": "sent",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(patched.ok, "{patched:?}");
    assert_eq!(patched.data.unwrap()["status"], "sent");
}

#[tokio::test]
async fn mcp_invoice_create_rejects_non_positive_amount() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();
    let env = invoice_create_draft(
        &json!({
            "company": co.to_string_lossy(),
            "party_id": party.id.to_string(),
            "kind": "sale",
            "lines": [{
                "description": "Bad",
                "amount_minor": 0,
                "currency": "DKK"
            }],
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(!env.ok);
}

#[tokio::test]
async fn mcp_invoice_create_eur_requires_fx_rate() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = upsert_party(
        &co,
        None,
        "EU Buyer".into(),
        klarbog_plugin_crm::PartyKind::Business,
        None,
        None,
    )
    .unwrap();
    let missing = invoice_create_draft(
        &json!({
            "company": co.to_string_lossy(),
            "party_id": party.id.to_string(),
            "kind": "sale",
            "lines": [{
                "description": "Consulting",
                "amount_minor": 10_000,
                "currency": "EUR"
            }],
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(!missing.ok);
    assert!(missing.errors.iter().any(|e| e.contains("fx_rate")));

    let created = invoice_create_draft(
        &json!({
            "company": co.to_string_lossy(),
            "party_id": party.id.to_string(),
            "kind": "sale",
            "lines": [{
                "description": "Consulting",
                "amount_minor": 10_000,
                "currency": "EUR"
            }],
            "fx_rate_to_dkk": "7.46",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(created.ok, "{created:?}");
    assert_eq!(
        created.data.unwrap()["invoice"]["fx_rate_to_dkk_micro"],
        7_460_000
    );
}

#[tokio::test]
async fn mcp_invoice_authz_denied() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let env = invoice_list(
        &json!({
            "company": co.to_string_lossy(),
            "actor_kind": "agent",
            "actor_id": "stranger",
        }),
        dir.path(),
    )
    .await;
    assert!(!env.ok);
}
