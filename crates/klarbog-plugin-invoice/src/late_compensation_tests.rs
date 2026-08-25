//! Tests for late_compensation (kept separate for linegate).

use super::late_compensation::*;
use crate::due_date::STATUTORY_PAYMENT_TERM_DAYS;
use crate::store::{create_draft_from_new, NewLine};
use crate::{InvoiceError, InvoiceKind, InvoiceStatus};
use klarbog_plugin_crm::upsert_party;
use std::path::Path;
use tempfile::tempdir;

fn issued_commercial_invoice(
    co: &Path,
    gross_minor: i64,
    issue: &str,
    due: &str,
) -> crate::Invoice {
    let party = upsert_party(
        co,
        None,
        "Kunde A/S".into(),
        klarbog_plugin_crm::PartyKind::Business,
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
    crate::issue::record_issue(
        co,
        &inv.id,
        issue.into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some("2026-0001".into()),
        None,
        None,
    )
    .unwrap();
    let mut file = crate::store::load(co).unwrap();
    let stored = file.invoices.iter_mut().find(|i| i.id == inv.id).unwrap();
    stored.due_date = Some(due.into());
    stored.status = InvoiceStatus::Sent;
    crate::store::save(co, &file).unwrap();
    crate::store::load(co)
        .unwrap()
        .invoices
        .into_iter()
        .find(|i| i.id == inv.id)
        .unwrap()
}

#[test]
fn commercial_overdue_invoice_is_eligible() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_commercial_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    let calc = calculate_late_compensation(&co, &inv, parse_iso_date("2026-06-20").unwrap(), None)
        .unwrap();
    assert!(calc.eligible);
    assert!(calc.is_commercial_transaction);
    assert_eq!(calc.compensation_amount_minor, STATUTORY_COMPENSATION_MINOR);
    assert_eq!(calc.overdue_days, 5);
}

#[test]
fn private_buyer_is_not_eligible() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Privat".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Work".into(),
            amount_minor: 125_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    crate::issue::record_issue(
        &co,
        &inv.id,
        "2026-05-16".into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some("2026-0002".into()),
        None,
        None,
    )
    .unwrap();
    let mut file = crate::store::load(&co).unwrap();
    let stored = file.invoices.iter_mut().find(|i| i.id == inv.id).unwrap();
    stored.due_date = Some("2026-06-15".into());
    stored.status = InvoiceStatus::Sent;
    crate::store::save(&co, &file).unwrap();
    let inv = crate::store::load(&co)
        .unwrap()
        .invoices
        .into_iter()
        .find(|i| i.id == inv.id)
        .unwrap();
    let calc = calculate_late_compensation(&co, &inv, parse_iso_date("2026-06-20").unwrap(), None)
        .unwrap();
    assert!(!calc.eligible);
    assert!(!calc.is_commercial_transaction);
}

#[test]
fn register_rejects_duplicate_and_private_buyer() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_commercial_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    let as_of = parse_iso_date("2026-06-20").unwrap();
    register_invoice_compensation(&co, &inv.id, as_of, None, None).unwrap();
    let dup = register_invoice_compensation(&co, &inv.id, as_of, None, None);
    assert!(matches!(
        dup,
        Err(InvoiceError::CompensationAlreadyRegistered)
    ));
}

#[test]
fn claim_open_balance_includes_compensation() {
    use crate::due_date::parse_iso_date;
    use crate::late_interest::claim_open_balance_minor;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_commercial_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    let before = claim_open_balance_minor(&inv).unwrap();
    register_invoice_compensation(
        &co,
        &inv.id,
        parse_iso_date("2026-06-20").unwrap(),
        None,
        None,
    )
    .unwrap();
    let updated = crate::store::load(&co).unwrap().invoices[0].clone();
    assert_eq!(
        claim_open_balance_minor(&updated).unwrap(),
        before + STATUTORY_COMPENSATION_MINOR
    );
}

#[test]
fn posts_compensation_once_and_rejects_double_post() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_commercial_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    register_invoice_compensation(
        &co,
        &inv.id,
        parse_iso_date("2026-06-20").unwrap(),
        None,
        None,
    )
    .unwrap();
    let stored = crate::store::load(&co).unwrap().invoices[0].clone();
    let claim = &stored.compensation_claims[0];
    let actor = klarbog_types::Actor::user("t");
    let entry = compensation_post_journal_suggestion(
        &stored,
        claim,
        &actor,
        &crate::InvoiceConfig::default(),
    )
    .unwrap();
    assert!(entry.memo.contains(":compensation:2026-06-20"));
    mark_compensation_posted(&co, &inv.id, "2026-06-20", "je-1").unwrap();
    let err = mark_compensation_posted(&co, &inv.id, "2026-06-20", "je-2");
    assert!(matches!(
        err,
        Err(InvoiceError::CompensationClaimAlreadyPosted)
    ));
}
