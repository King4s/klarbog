//! Dual-write claim register → SQLite + audit_log (linegate split).

use crate::due_date::{parse_iso_date, STATUTORY_PAYMENT_TERM_DAYS};
use crate::late_compensation::{
    mark_compensation_posted, register_invoice_compensation, STATUTORY_COMPENSATION_MINOR,
};
use crate::late_interest::{mark_interest_claim_posted, register_late_interest};
use crate::reminders::{mark_reminder_posted, register_invoice_reminder, MAX_REMINDER_FEE_MINOR};
use crate::store::{create_draft_from_new, NewLine};
use crate::{InvoiceError, InvoiceKind, InvoiceStatus};
use klarbog_plugin_crm::{upsert_party, PartyKind};
use klarbog_store_sqlite::{
    open_company, COMPENSATION_POST_AUDIT, COMPENSATION_REGISTER_AUDIT, INTEREST_POST_AUDIT,
    INTEREST_REGISTER_AUDIT, REMINDER_POST_AUDIT, REMINDER_REGISTER_AUDIT,
};
use std::path::Path;
use tempfile::tempdir;

fn issued(co: &Path, kind: PartyKind, gross_minor: i64, issue: &str, due: &str) -> crate::Invoice {
    let party = upsert_party(co, None, "Buyer".into(), kind, None, None).unwrap();
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
fn reminder_register_writes_sqlite_row_and_audit() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued(&co, PartyKind::Private, 125_000, "2026-05-16", "2026-06-15");
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-26").unwrap(),
        None,
        None,
    )
    .unwrap();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let store = open_company(&co).await.unwrap();
        assert_eq!(store.count_reminder_claims().await.unwrap(), 1);
        let audits = store
            .list_audit_by_event(REMINDER_REGISTER_AUDIT)
            .await
            .unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].entity_id.as_deref(), Some(inv.id.as_str()));
        assert!(audits[0]
            .message
            .contains(&MAX_REMINDER_FEE_MINOR.to_string()));
    });
}

#[test]
fn reminder_re_register_failure_does_not_double_sqlite() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued(&co, PartyKind::Private, 125_000, "2026-05-16", "2026-06-01");
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

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let store = open_company(&co).await.unwrap();
        assert_eq!(store.count_reminder_claims().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(REMINDER_REGISTER_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
    });
}

#[test]
fn interest_register_writes_sqlite_and_rejects_dup_without_double() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued(&co, PartyKind::Private, 125_000, "2026-05-16", "2026-06-15");
    let as_of = parse_iso_date("2026-06-20").unwrap();
    register_late_interest(&co, &inv.id, as_of, Some(220), None).unwrap();
    let dup = register_late_interest(&co, &inv.id, as_of, Some(220), None);
    assert!(matches!(
        dup,
        Err(InvoiceError::DuplicateInterestClaim { .. })
    ));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let store = open_company(&co).await.unwrap();
        assert_eq!(store.count_interest_claims().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(INTEREST_REGISTER_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
    });
}

#[test]
fn compensation_register_writes_sqlite_and_rejects_dup_without_double() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued(
        &co,
        PartyKind::Business,
        125_000,
        "2026-05-16",
        "2026-06-15",
    );
    let as_of = parse_iso_date("2026-06-20").unwrap();
    register_invoice_compensation(&co, &inv.id, as_of, None, None).unwrap();
    let dup = register_invoice_compensation(&co, &inv.id, as_of, None, None);
    assert!(matches!(
        dup,
        Err(InvoiceError::CompensationAlreadyRegistered)
    ));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let store = open_company(&co).await.unwrap();
        assert_eq!(store.count_compensation_claims().await.unwrap(), 1);
        let audits = store
            .list_audit_by_event(COMPENSATION_REGISTER_AUDIT)
            .await
            .unwrap();
        assert_eq!(audits.len(), 1);
        assert!(audits[0]
            .message
            .contains(&STATUTORY_COMPENSATION_MINOR.to_string()));
    });
}

#[test]
fn reminder_post_writes_sqlite_link_and_rejects_double() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued(&co, PartyKind::Private, 125_000, "2026-05-16", "2026-06-15");
    register_invoice_reminder(
        &co,
        &inv.id,
        parse_iso_date("2026-06-26").unwrap(),
        None,
        None,
    )
    .unwrap();
    mark_reminder_posted(&co, &inv.id, "2026-06-26", "je_r1").unwrap();
    let err = mark_reminder_posted(&co, &inv.id, "2026-06-26", "je_r2").unwrap_err();
    assert!(matches!(err, InvoiceError::ReminderAlreadyPosted));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let store = open_company(&co).await.unwrap();
        assert_eq!(store.count_reminder_postings().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(REMINDER_POST_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
    });
}

#[test]
fn interest_post_writes_sqlite_link_and_rejects_double() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued(&co, PartyKind::Private, 125_000, "2026-05-16", "2026-06-15");
    register_late_interest(
        &co,
        &inv.id,
        parse_iso_date("2026-06-20").unwrap(),
        Some(220),
        None,
    )
    .unwrap();
    mark_interest_claim_posted(&co, &inv.id, "2026-06-20", Some(220), "je_i1").unwrap();
    let err =
        mark_interest_claim_posted(&co, &inv.id, "2026-06-20", Some(220), "je_i2").unwrap_err();
    assert!(matches!(err, InvoiceError::InterestClaimAlreadyPosted));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let store = open_company(&co).await.unwrap();
        assert_eq!(store.count_interest_postings().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(INTEREST_POST_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
    });
}

#[test]
fn compensation_post_writes_sqlite_link_and_rejects_double() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued(
        &co,
        PartyKind::Business,
        125_000,
        "2026-05-16",
        "2026-06-15",
    );
    register_invoice_compensation(
        &co,
        &inv.id,
        parse_iso_date("2026-06-20").unwrap(),
        None,
        None,
    )
    .unwrap();
    mark_compensation_posted(&co, &inv.id, "2026-06-20", "je_c1").unwrap();
    let err = mark_compensation_posted(&co, &inv.id, "2026-06-20", "je_c2").unwrap_err();
    assert!(matches!(err, InvoiceError::CompensationClaimAlreadyPosted));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let store = open_company(&co).await.unwrap();
        assert_eq!(store.count_compensation_postings().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(COMPENSATION_POST_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
    });
}
