//! Unit tests for backup manifest (slice 36 linegate split).

use super::*;
use crate::ensure_company_extras;
use chrono::Utc;
use klarbog_core::init_company;
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_plugin_crm::upsert_party;
use klarbog_types::{Actor, Currency, MinorAmount};
use tempfile::tempdir;

fn expense(actor: Actor, minor: i64) -> JournalEntry {
    let amount = MinorAmount::from_minor(minor);
    let currency = Currency::new("DKK").unwrap();
    JournalEntry {
        as_of: Utc::now(),
        memo: "backup test".into(),
        actor,
        legs: vec![
            Leg {
                account: "3000".into(),
                direction: Direction::Debit,
                amount,
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: "2000".into(),
                direction: Direction::Credit,
                amount,
                currency,
                party_id: None,
            },
        ],
    }
}

#[tokio::test]
async fn manifest_lists_files_and_digests() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    let actor = Actor::user("owner");
    init_company(&co, "Backup Test ApS", &actor).await.unwrap();
    ensure_company_extras(&co).unwrap();
    upsert_party(
        &co,
        None,
        "Vendor".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let company = klarbog_core::open_existing(&co).await.unwrap();
    let posted = company.post(expense(actor, 500)).await.unwrap();
    assert!(!posted.digest.is_empty());

    let manifest = write_backup_manifest(&co).await.unwrap();
    assert!(manifest.files.iter().any(|f| f.path == "policy.json"));
    assert!(manifest.files.iter().any(|f| f.path == "retention.json"));
    assert_eq!(manifest.journal_digests.len(), 1);
    assert_eq!(manifest.parties.len(), 1);
    let path = manifest_path(&co, &manifest.backup_key);
    assert!(path.exists());
    assert!(manifest
        .content_sha256
        .as_ref()
        .is_some_and(|h| h.len() == 64));
    let sidecar = manifest_sidecar_path(&path);
    assert!(sidecar.exists());
    assert!(verify_manifest_sidecar(&path));
}

#[tokio::test]
async fn sidecar_detects_tamper() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    let actor = Actor::user("owner");
    init_company(&co, "Tamper Test", &actor).await.unwrap();
    ensure_company_extras(&co).unwrap();
    let manifest = write_backup_manifest(&co).await.unwrap();
    let path = manifest_path(&co, &manifest.backup_key);
    assert!(verify_manifest_sidecar(&path));
    let mut text = fs::read_to_string(&path).unwrap();
    text.push(' ');
    fs::write(&path, text).unwrap();
    assert!(!verify_manifest_sidecar(&path));
}

#[tokio::test]
async fn manifest_includes_invoice_payments_summary_when_present() {
    use klarbog_plugin_invoice::{
        create_draft_from_new, mark_part_paid_preview, patch_status, InvoiceConfig, InvoiceKind,
        InvoiceStatus, NewLine,
    };

    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    let actor = Actor::user("owner");
    init_company(&co, "Pay Backup", &actor).await.unwrap();
    ensure_company_extras(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let unpaid = create_draft_from_new(
        &co,
        party.id.clone(),
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Open".into(),
            amount_minor: 5_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let paid = create_draft_from_new(
        &co,
        party.id.clone(),
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Partial".into(),
            amount_minor: 10_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    patch_status(&co, &paid.id, InvoiceStatus::Sent).unwrap();
    mark_part_paid_preview(&co, &paid.id, 4_000, &actor, &InvoiceConfig::default()).unwrap();

    let manifest = build_backup_manifest(&co).await.unwrap();
    assert_eq!(manifest.invoices.len(), 2);
    let unpaid_ref = manifest
        .invoices
        .iter()
        .find(|i| i.id == unpaid.id.to_string())
        .unwrap();
    assert!(unpaid_ref.payments.is_none());
    let paid_ref = manifest
        .invoices
        .iter()
        .find(|i| i.id == paid.id.to_string())
        .unwrap();
    let summary = paid_ref.payments.as_ref().unwrap();
    assert_eq!(summary.count, 1);
    assert_eq!(summary.total_minor, 4_000);
}
