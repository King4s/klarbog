//! Tests for reminders (kept separate for linegate).

use super::reminders::*;
use crate::due_date::STATUTORY_PAYMENT_TERM_DAYS;
use crate::store::{create_draft_from_new, NewLine};
use crate::{InvoiceConfig, InvoiceError, InvoiceKind, InvoiceStatus};
use klarbog_plugin_crm::upsert_party;
use klarbog_types::Actor;
use std::path::Path;
use tempfile::tempdir;

fn issued_invoice(co: &Path, gross_minor: i64, issue: &str, due: &str) -> crate::Invoice {
    let party = upsert_party(
        co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
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
fn registers_statutory_reminder_fee_on_overdue_invoice() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    let (_, result) = register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-26").unwrap(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(result.reminder_sequence, 1);
    assert_eq!(result.fee_amount_minor, MAX_REMINDER_FEE_MINOR);
    assert_eq!(result.total_reminder_fees_minor, MAX_REMINDER_FEE_MINOR);
    assert_eq!(
        claim_open_balance_minor(&inv).unwrap() + MAX_REMINDER_FEE_MINOR,
        {
            let updated = crate::store::load(&co).unwrap();
            claim_open_balance_minor(&updated.invoices[0]).unwrap()
        }
    );
}

#[test]
fn posts_reminder_once_and_rejects_double_post() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-26").unwrap(),
        None,
        None,
    )
    .unwrap();
    let stored = crate::store::load(&co).unwrap().invoices[0].clone();
    let reminder = &stored.reminders[0];
    let actor = Actor::user("t");
    let cfg = InvoiceConfig::default();
    let entry = reminder_post_journal_suggestion(&stored, reminder, &actor, &cfg).unwrap();
    assert_eq!(entry.legs.len(), 2);
    assert_eq!(entry.legs[0].account.as_str(), "1100");
    assert_eq!(entry.legs[0].direction, klarbog_journal::Direction::Debit);
    assert_eq!(entry.legs[0].amount.minor(), MAX_REMINDER_FEE_MINOR);
    assert_eq!(entry.legs[1].account.as_str(), "1010");
    assert_eq!(entry.legs[1].direction, klarbog_journal::Direction::Credit);

    mark_reminder_posted(&co, &inv.id, "2026-06-26", "je_test_1").unwrap();
    let err = mark_reminder_posted(&co, &inv.id, "2026-06-26", "je_test_2").unwrap_err();
    assert!(matches!(err, InvoiceError::ReminderAlreadyPosted));

    let posted = crate::store::load(&co).unwrap().invoices[0].clone();
    let err =
        reminder_post_journal_suggestion(&posted, &posted.reminders[0], &actor, &cfg).unwrap_err();
    assert!(matches!(err, InvoiceError::ReminderAlreadyPosted));
}

#[test]
fn blocks_fourth_reminder() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-01");
    let dates = ["2026-06-11", "2026-06-22", "2026-07-03"];
    for d in dates {
        register_invoice_reminder(&co, &inv.id, parse_iso_date(d).unwrap(), None, None).unwrap();
    }
    let err = register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-07-14").unwrap(),
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, InvoiceError::MaxRemindersReached { max: 3 }));
}

#[test]
fn blocks_reminder_sent_too_soon() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-01");
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-11").unwrap(),
        None,
        None,
    )
    .unwrap();
    let err = register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-20").unwrap(),
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, InvoiceError::ReminderTooSoon { .. }));
}

#[test]
fn posts_specific_newer_reminder_leaving_older_unposted() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-01");
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-11").unwrap(),
        None,
        None,
    )
    .unwrap();
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-22").unwrap(),
        None,
        None,
    )
    .unwrap();
    let stored = crate::store::load(&co).unwrap().invoices[0].clone();
    let (idx, rem) = resolve_unposted_reminder(&stored, Some("2026-06-22"), None).unwrap();
    assert_eq!(idx, 1);
    assert_eq!(rem.reminder_date, "2026-06-22");
    let actor = Actor::user("t");
    let cfg = InvoiceConfig::default();
    let entry = reminder_post_journal_suggestion(&stored, rem, &actor, &cfg).unwrap();
    assert!(entry.memo.contains(":reminder:2026-06-22"));
    mark_reminder_posted(&co, &inv.id, "2026-06-22", "je_newer").unwrap();
    let after = crate::store::load(&co).unwrap().invoices[0].clone();
    assert!(after.reminders[0].posted_journal_id.is_none());
    assert_eq!(
        after.reminders[1].posted_journal_id.as_deref(),
        Some("je_newer")
    );
    let (oldest_idx, oldest) = resolve_unposted_reminder(&after, None, None).unwrap();
    assert_eq!(oldest_idx, 0);
    assert_eq!(oldest.reminder_date, "2026-06-11");
    let err = resolve_unposted_reminder(&after, Some("2026-06-22"), None).unwrap_err();
    assert!(matches!(err, InvoiceError::ReminderAlreadyPosted));
}

#[test]
fn resolve_reminder_by_sequence() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-01");
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-11").unwrap(),
        None,
        None,
    )
    .unwrap();
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-22").unwrap(),
        None,
        None,
    )
    .unwrap();
    let stored = crate::store::load(&co).unwrap().invoices[0].clone();
    let (_, rem) = resolve_unposted_reminder(&stored, None, Some(2)).unwrap();
    assert_eq!(rem.reminder_date, "2026-06-22");
}

use crate::late_interest::claim_open_balance_minor;
