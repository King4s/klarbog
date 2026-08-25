//! MCP invoice_send_email tests (DK-EMAIL-DELIVERY-001).

use super::*;
use klarbog_core::init_company;
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{
    create_draft_from_new, due_date::STATUTORY_PAYMENT_TERM_DAYS, read_send_log, record_issue,
    InvoiceKind, NewLine,
};
use klarbog_types::Actor;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn issued_fixture(co: &std::path::Path, email: Option<&str>) -> klarbog_plugin_invoice::InvoiceId {
    let party = upsert_party(
        co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        email.map(|e| e.to_string()),
    )
    .unwrap();
    let inv = create_draft_from_new(
        co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Widget".into(),
            amount_minor: 12_500,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let invoice_no = "2026-0042";
    let object_dir = co.join("objects/invoices/issued");
    fs::create_dir_all(&object_dir).unwrap();
    fs::write(
        object_dir.join(format!("{invoice_no}.json")),
        br#"{"type":"issued_invoice","invoiceNumber":"2026-0042"}"#,
    )
    .unwrap();
    record_issue(
        co,
        &inv.id,
        "2026-05-16".into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some(invoice_no.into()),
        Some("doc_mcp_email".into()),
        None,
    )
    .unwrap();
    inv.id
}

#[tokio::test]
async fn mcp_invoice_send_email_requires_confirm() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_fixture(&co, Some("buyer@example.com"));
    let env = invoice_send_email(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "to": "buyer@example.com",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(!env.ok);
    assert!(env.errors.join(" ").contains("confirm"));
}

#[tokio::test]
async fn mcp_invoice_send_email_by_invoice_number_dry_run() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    issued_fixture(&co, Some("buyer@example.com"));
    let env = invoice_send_email(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_number": "2026-0042",
            "to": "buyer@example.com",
            "confirm": true,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(env.ok, "{env:?}");
    let data = env.data.unwrap();
    assert_eq!(data["recipient"], "buyer@example.com");
    assert_eq!(data["kind"], "invoice");
    assert_eq!(data["duplicate"], false);
    assert_eq!(read_send_log(&co).unwrap().len(), 1);
}

#[tokio::test]
async fn mcp_invoice_send_email_idempotent() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_fixture(&co, Some("buyer@example.com"));
    let args = json!({
        "company": co.to_string_lossy(),
        "invoice_id": inv_id.to_string(),
        "to": "buyer@example.com",
        "confirm": true,
        "actor_kind": "user",
        "actor_id": "owner",
    });
    assert!(invoice_send_email(&args, dir.path()).await.ok);
    let second = invoice_send_email(&args, dir.path()).await;
    assert!(second.ok, "{second:?}");
    assert_eq!(second.data.unwrap()["duplicate"], true);
    assert_eq!(read_send_log(&co).unwrap().len(), 1);
}
