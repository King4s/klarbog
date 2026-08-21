//! Unit tests for invoice plugin (types + lifecycle previews).

use super::*;
use klarbog_plugin_crm::{upsert_party, PARTIES_FILENAME};
use std::fs;
use tempfile::tempdir;

#[test]
fn invoice_has_no_journal_write() {
    let p = InvoicePlugin;
    assert!(!p.has_journal_write());
}

#[test]
fn draft_roundtrip_with_journal_suggestion() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Nordic Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    assert!(co.join(PARTIES_FILENAME).exists());
    let plugin = InvoicePlugin;
    let invoice = plugin
        .create(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Support".into(),
                amount_minor: 5000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
    let entry = journal_suggestion(
        &invoice,
        &klarbog_types::Actor::user("t"),
        &InvoiceConfig::default(),
    )
    .unwrap();
    assert!(entry
        .legs
        .iter()
        .all(|l| l.party_id.as_ref() == Some(&invoice.party_id)));
    assert_eq!(plugin.list(&co).unwrap().len(), 1);
    assert_eq!(invoice.status, InvoiceStatus::Draft);
}

#[test]
fn lifecycle_patch_and_mark_paid() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let plugin = InvoicePlugin;
    let invoice = plugin
        .create(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Item".into(),
                amount_minor: 3000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
    let sent = patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
    assert_eq!(sent.status, InvoiceStatus::Sent);
    let actor = klarbog_types::Actor::user("t");
    let (paid, entry) =
        mark_paid_preview(&co, &invoice.id, &actor, &InvoiceConfig::default()).unwrap();
    assert_eq!(paid.status, InvoiceStatus::Paid);
    assert!(entry
        .legs
        .iter()
        .all(|l| l.party_id.as_ref() == Some(&invoice.party_id)));
}

#[test]
fn lifecycle_mark_part_paid() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let plugin = InvoicePlugin;
    let invoice = plugin
        .create(
            &co,
            party.id.clone(),
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Item".into(),
                amount_minor: 8000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
    patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
    let actor = klarbog_types::Actor::user("t");
    let (part, entry) =
        mark_part_paid_preview(&co, &invoice.id, 2500, &actor, &InvoiceConfig::default()).unwrap();
    assert_eq!(part.status, InvoiceStatus::PartPaid);
    assert_eq!(part.payments.len(), 1);
    assert_eq!(entry.legs[0].amount.minor(), 2500);
    assert_eq!(part.remaining_minor().unwrap(), 5500);
    assert!(entry
        .legs
        .iter()
        .all(|l| l.party_id.as_ref() == Some(&party.id)));
}
