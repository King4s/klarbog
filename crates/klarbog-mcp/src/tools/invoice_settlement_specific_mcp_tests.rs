//! MCP: post a specific interest claim when several are unposted (linegate split).

use super::*;
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_plugin_crm::{upsert_party, PartyKind};
use klarbog_plugin_invoice::{
    create_draft_from_new, due_date::STATUTORY_PAYMENT_TERM_DAYS, patch_status, record_issue,
    InvoiceKind, InvoiceStatus, NewLine,
};
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;

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
async fn mcp_post_specific_interest_claim_preview() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    init_company(&co, "Demo", &Actor::user("owner"))
        .await
        .unwrap();
    let inv_id = issued_private(&co, 125_000, "2026-05-16");
    for as_of in ["2026-06-20", "2026-07-01"] {
        assert!(
            invoice_claim_interest(
                &json!({
                    "company": co.to_string_lossy(),
                    "invoice_id": inv_id.to_string(),
                    "as_of": as_of,
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
    }
    let store = ConfirmStore::default();
    let registry = default_registry();
    let specific = invoice_post_interest_preview(
        &json!({
            "company": co.to_string_lossy(),
            "invoice_id": inv_id.to_string(),
            "claim_date": "2026-07-01",
            "reference_rate_bps": 220,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
        &store,
        &registry,
    )
    .await;
    assert!(specific.ok, "{specific:?}");
    let memo = specific.data.unwrap()["journal_entry"]["memo"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        memo.contains(":interest:2026-07-01@220"),
        "expected specific claim memo, got {memo}"
    );
    let default = invoice_post_interest_preview(
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
    assert!(default.ok, "{default:?}");
    let oldest = default.data.unwrap()["journal_entry"]["memo"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        oldest.contains(":interest:2026-06-20@"),
        "default should post oldest, got {oldest}"
    );
}
