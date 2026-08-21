use crate::reconcile::*;
use crate::{apply_match, BankRow};
use chrono::{TimeZone, Utc};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use klarbog_types::MinorAmount;
use std::fs;
use tempfile::tempdir;

fn row(text: &str, minor: i64) -> BankRow {
    BankRow {
        date: Utc.with_ymd_and_hms(2026, 5, 20, 0, 0, 0).unwrap(),
        text: text.into(),
        amount_minor: MinorAmount::from_minor(minor),
    }
}

#[test]
fn matches_sale_by_amount_and_party_name() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Nordic Supply".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Consulting".into(),
            amount_minor: 50_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();

    let rows = vec![row("Customer payment Nordic Supply consulting", 50_000)];
    let results = suggest_matches(&co, &rows).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].suggestions.len(), 1);
    assert!(results[0].suggestions[0].confidence_bps >= SAFE_THRESHOLD_BPS);
    assert_eq!(results[0].suggestions[0].kind, MatchKind::SaleInvoice);
}

#[test]
fn purchase_outgoing_match() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Nordic Supply".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Purchase,
        vec![NewLine {
            description: "Office goods".into(),
            amount_minor: 12_550,
            currency: "DKK".into(),
        }],
    )
    .unwrap();

    let rows = vec![row("Payment Nordic Supply office", -12_550)];
    let results = suggest_matches(&co, &rows).unwrap();
    assert_eq!(results[0].suggestions.len(), 1);
    assert_eq!(results[0].suggestions[0].kind, MatchKind::PurchaseInvoice);
}

#[test]
fn amount_only_stays_below_safe_and_sets_unsafe_reason() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Hidden Vendor".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Purchase,
        vec![NewLine {
            description: "Secret".into(),
            amount_minor: 9_999,
            currency: "DKK".into(),
        }],
    )
    .unwrap();

    let rows = vec![row("Wire transfer xyz", -9_999)];
    let results = suggest_matches(&co, &rows).unwrap();
    assert!(results[0].suggestions.is_empty());
    assert!(results[0].unsafe_match_reason.is_some());
}

#[test]
fn sync_raises_unmatched_exception_once() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let rows = vec![row("Mystery deposit", 1_000)];
    let results = suggest_matches(&co, &rows).unwrap();
    let raised = sync_unmatched_exceptions(&co, &rows, &results).unwrap();
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0].code, EXCEPTION_UNMATCHED_BANK);
    let again = sync_unmatched_exceptions(&co, &rows, &results).unwrap();
    assert!(again.is_empty());
    assert_eq!(list_unmatched_bank_exceptions(&co).unwrap().len(), 1);
}

#[test]
fn apply_safe_match_returns_payment_suggestion() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Nordic Supply".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Consulting".into(),
            amount_minor: 50_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let bank = row("Customer payment Nordic Supply consulting", 50_000);
    let actor = klarbog_types::Actor::user("owner");
    let applied = apply_match(&co, &bank, inv.id.as_str(), &actor, false, 0).unwrap();
    assert!(!applied.forced);
    assert!(applied.confidence_bps >= SAFE_THRESHOLD_BPS);
    assert!(applied.entry.memo.contains("bank:"));
    assert!(applied.entry.memo.contains(inv.id.as_str()));
    assert!(applied.entry.legs.iter().all(|l| l.party_id.is_some()));
    applied.entry.validate().unwrap();
}

#[test]
fn apply_rejects_unsafe_without_force() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Hidden Vendor".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Purchase,
        vec![NewLine {
            description: "Secret".into(),
            amount_minor: 9_999,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let bank = row("Wire transfer xyz", -9_999);
    let actor = klarbog_types::Actor::user("owner");
    let err = apply_match(&co, &bank, inv.id.as_str(), &actor, false, 0).unwrap_err();
    assert!(matches!(err, ReconcileError::UnsafeMatch { .. }));
}

#[test]
fn apply_force_user_closes_unmatched_exception() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Hidden Vendor".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Purchase,
        vec![NewLine {
            description: "Secret".into(),
            amount_minor: 9_999,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let bank = row("Wire transfer xyz", -9_999);
    let results = suggest_matches(&co, std::slice::from_ref(&bank)).unwrap();
    sync_unmatched_exceptions(&co, std::slice::from_ref(&bank), &results).unwrap();
    assert_eq!(list_unmatched_bank_exceptions(&co).unwrap().len(), 1);

    let agent = klarbog_types::Actor::agent("bot");
    let denied = apply_match(&co, &bank, inv.id.as_str(), &agent, true, 0).unwrap_err();
    assert!(matches!(denied, ReconcileError::ForceRequiresUser));

    let user = klarbog_types::Actor::user("owner");
    let applied = apply_match(&co, &bank, inv.id.as_str(), &user, true, 0).unwrap();
    assert!(applied.forced);
    assert!(applied.exception_closed.is_some());
    assert!(list_unmatched_bank_exceptions(&co).unwrap().is_empty());
}
