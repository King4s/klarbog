//! MCP tests: mark-paid / mark-part-paid preview (suggestion-only + ConfirmStore).

use super::*;
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{
    create_draft_from_new, patch_status, InvoiceKind, InvoiceStatus, NewLine,
};
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn mcp_mark_paid_preview() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = upsert_party(&co, None, "Buyer".into()).unwrap();
    let invoice = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Item".into(),
            amount_minor: 1000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
    let args = json!({
        "company": co.to_string_lossy(),
        "invoice_id": invoice.id.to_string(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = invoice_mark_paid_preview(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "paid");
    assert!(data["journal_entry"]["legs"].as_array().unwrap()[0]["party_id"].is_string());
    assert!(data.get("confirm_token").is_none());
}

#[tokio::test]
async fn mcp_mark_paid_preview_issues_confirm_token() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = upsert_party(&co, None, "Buyer".into()).unwrap();
    let invoice = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Item".into(),
            amount_minor: 1000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
    let args = json!({
        "company": co.to_string_lossy(),
        "invoice_id": invoice.id.to_string(),
        "preview": true,
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = invoice_mark_paid_preview(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "paid");
    let token = data["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(data["payload_digest"].as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn mcp_mark_part_paid_preview() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = upsert_party(&co, None, "Buyer".into()).unwrap();
    let invoice = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Item".into(),
            amount_minor: 10_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
    let args = json!({
        "company": co.to_string_lossy(),
        "invoice_id": invoice.id.to_string(),
        "amount_minor": 3_000,
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = invoice_mark_part_paid_preview(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "part_paid");
    assert_eq!(data["invoice"]["payments"].as_array().unwrap().len(), 1);
    assert!(data["journal_entry"]["legs"].as_array().unwrap()[0]["party_id"].is_string());
    let amount = &data["journal_entry"]["legs"][0]["amount"];
    let units = amount
        .as_i64()
        .or_else(|| amount.get("units").and_then(|u| u.as_i64()));
    assert_eq!(units, Some(3_000));
    assert!(data.get("confirm_token").is_none());

    let paid_args = json!({
        "company": co.to_string_lossy(),
        "invoice_id": invoice.id.to_string(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let paid_env = invoice_mark_paid_preview(&paid_args, dir.path(), &store, &registry).await;
    assert!(paid_env.ok);
    let paid = paid_env.data.unwrap();
    assert_eq!(paid["invoice"]["status"], "paid");
    assert_eq!(paid["invoice"]["payments"].as_array().unwrap().len(), 2);
    let rem = &paid["journal_entry"]["legs"][0]["amount"];
    let rem_units = rem
        .as_i64()
        .or_else(|| rem.get("units").and_then(|u| u.as_i64()));
    assert_eq!(rem_units, Some(7_000));
}

#[tokio::test]
async fn mcp_mark_part_paid_preview_issues_confirm_token() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let owner = Actor::user("owner");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = upsert_party(&co, None, "Buyer".into()).unwrap();
    let invoice = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Item".into(),
            amount_minor: 10_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
    let args = json!({
        "company": co.to_string_lossy(),
        "invoice_id": invoice.id.to_string(),
        "amount_minor": 3_000,
        "preview": true,
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = invoice_mark_part_paid_preview(&args, dir.path(), &store, &registry).await;
    assert!(env.ok);
    let data = env.data.unwrap();
    assert_eq!(data["invoice"]["status"], "part_paid");
    assert_eq!(
        data["invoice"]["payments"][0]["amount_minor"].as_i64(),
        Some(3_000)
    );
    let token = data["confirm_token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert!(data["expires_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(data["payload_digest"].as_str().unwrap().len(), 64);
}
