//! MCP tests: late compensation and late interest (calc, claim, post preview).

use super::*;
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_crm::{upsert_party, PartyKind};
use klarbog_plugin_invoice::{
    create_draft_from_new, due_date::STATUTORY_PAYMENT_TERM_DAYS, patch_status, record_issue,
    InvoiceKind, InvoiceStatus, NewLine, STATUTORY_COMPENSATION_MINOR,
};
use klarbog_types::Actor;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn issued_commercial(
    co: &std::path::Path,
    gross_minor: i64,
    issue: &str,
    invoice_no: &str,
) -> klarbog_plugin_invoice::InvoiceId {
    let party = upsert_party(
        co,
        None,
        "Kunde A/S".into(),
        PartyKind::Business,
        None,
        None,
    )
    .unwrap();
    let inv = create_draft_from_new(
        co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Work".into(),
            amount_minor: gross_minor,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let object_dir = co.join("objects/invoices/issued");
    fs::create_dir_all(&object_dir).unwrap();
    fs::write(
        object_dir.join(format!("{invoice_no}.json")),
        br#"{"type":"issued_invoice"}"#,
    )
    .unwrap();
    record_issue(
        co,
        &inv.id,
        issue.into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some(invoice_no.into()),
        None,
        None,
    )
    .unwrap();
    patch_status(co, &inv.id, InvoiceStatus::Sent).unwrap();
    inv.id
}

fn issued_private(
    co: &std::path::Path,
    gross_minor: i64,
    issue: &str,
) -> klarbog_plugin_invoice::InvoiceId {
    let party = upsert_party(co, None, "Buyer".into(), PartyKind::Private, None, None).unwrap();
    let inv = create_draft_from_new(
        co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Work".into(),
            amount_minor: gross_minor,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    record_issue(
        co,
        &inv.id,
        issue.into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some("2026-0002".into()),
        None,
        None,
    )
    .unwrap();
    patch_status(co, &inv.id, InvoiceStatus::Sent).unwrap();
    inv.id
}

#[tokio::test]
async fn mcp_compensation_calc_overdue_commercial() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_commercial(&co, 125_000, "2026-05-16", "2026-0100");
    let env = invoice_compensation_calc(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "as_of": "2026-06-20",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(env.ok, "{env:?}");
    let calc = &env.data.unwrap()["calculation"];
    assert_eq!(calc["eligible"], true);
    assert_eq!(
        calc["compensation_amount_minor"].as_i64(),
        Some(STATUTORY_COMPENSATION_MINOR)
    );
}

#[tokio::test]
async fn mcp_compensation_calc_by_invoice_number() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    issued_commercial(&co, 125_000, "2026-05-16", "2026-0101");
    let env = invoice_compensation_calc(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_number": "2026-0101",
            "as_of": "2026-06-20",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(env.ok, "{env:?}");
}

#[tokio::test]
async fn mcp_claim_compensation_requires_confirm() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_commercial(&co, 125_000, "2026-05-16", "2026-0102");
    let env = invoice_claim_compensation(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "as_of": "2026-06-20",
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
async fn mcp_claim_and_post_compensation_preview() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_commercial(&co, 125_000, "2026-05-16", "2026-0103");
    assert!(
        invoice_claim_compensation(
            &json!({
                "company": co.to_string_lossy(),
                "invoice_id": inv_id.to_string(),
                "as_of": "2026-06-20",
                "confirm": true,
                "actor_kind": "user",
                "actor_id": "owner",
            }),
            dir.path(),
        )
        .await
        .ok
    );
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = invoice_post_compensation_preview(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
        &store,
        &registry,
    )
    .await;
    assert!(env.ok, "{env:?}");
    let data = env.data.unwrap();
    assert!(data["journal_entry"]["memo"]
        .as_str()
        .unwrap()
        .contains(":compensation:"));
    assert!(data.get("confirm_token").is_none());

    let preview = invoice_post_compensation_preview(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "preview": true,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
        &store,
        &registry,
    )
    .await;
    assert!(preview.ok, "{preview:?}");
    assert!(
        preview.data.unwrap()["confirm_token"]
            .as_str()
            .unwrap()
            .len()
            > 10
    );
}

fn issued_private_overdue(co: &std::path::Path) -> klarbog_plugin_invoice::InvoiceId {
    issued_private(co, 125_000, "2026-05-16")
}

#[tokio::test]
async fn mcp_interest_calc_overdue() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_private_overdue(&co);
    let env = invoice_interest_calc(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "as_of": "2026-06-20",
            "reference_rate": 2.2,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(env.ok, "{env:?}");
    let calc = &env.data.unwrap()["calculation"];
    assert_eq!(calc["overdue_days"].as_u64(), Some(5));
    assert_eq!(calc["reference_rate_bps"].as_i64(), Some(220));
    assert_eq!(calc["accrued_interest_minor"].as_i64(), Some(175));
}

#[tokio::test]
async fn mcp_claim_interest_requires_confirm() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_private(&co, 125_000, "2026-05-16");
    let env = invoice_claim_interest(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "as_of": "2026-06-20",
            "reference_rate": 2.2,
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
async fn mcp_claim_and_post_interest_preview() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_private(&co, 125_000, "2026-05-16");
    assert!(
        invoice_claim_interest(
            &json!({
                "company": co.to_string_lossy(),
                "invoice_id": inv_id.to_string(),
                "as_of": "2026-06-20",
                "reference_rate": 2.2,
                "confirm": true,
                "actor_kind": "user",
                "actor_id": "owner",
            }),
            dir.path(),
        )
        .await
        .ok
    );
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = invoice_post_interest_preview(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
        &store,
        &registry,
    )
    .await;
    assert!(env.ok, "{env:?}");
    let data = env.data.unwrap();
    assert!(data["journal_entry"]["memo"]
        .as_str()
        .unwrap()
        .contains(":interest:"));
    assert!(data.get("confirm_token").is_none());

    let preview = invoice_post_interest_preview(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "preview": true,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
        &store,
        &registry,
    )
    .await;
    assert!(preview.ok, "{preview:?}");
    assert!(
        preview.data.unwrap()["confirm_token"]
            .as_str()
            .unwrap()
            .len()
            > 10
    );
}

#[tokio::test]
async fn mcp_post_compensation_preview_without_claim_fails() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_commercial(&co, 125_000, "2026-05-16", "2026-0104");
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = invoice_post_compensation_preview(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
        &store,
        &registry,
    )
    .await;
    assert!(!env.ok);
    assert!(env.errors.join(" ").contains("unposted compensation"));
}
