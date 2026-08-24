//! Unit tests for GDPR party erasure (wave31 soft linegate split).

use super::*;
use klarbog_core::{init_company, open_existing};
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_plugin_crm::upsert_party as crm_upsert;
use klarbog_plugin_documents::{attach_document, list_documents, DocumentKind};
use klarbog_types::{Actor, Currency, MinorAmount};
use tempfile::tempdir;

#[tokio::test]
async fn dry_run_does_not_mutate() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let co = dir.path().join("co");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = crm_upsert(
        &co,
        None,
        "Person A".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    attach_document(
        &co,
        DocumentKind::Receipt,
        "a.pdf".into(),
        Some(party.id.clone()),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let report = erase_party(&co, &party.id, ErasePartyOptions::default())
        .await
        .unwrap();
    assert!(report.dry_run);
    assert_eq!(report.display_name_before, "Person A");
    assert_eq!(report.documents_stripped.len(), 1);
    assert!(report.documents_deleted.is_empty());
    assert_eq!(
        get_party(&co, &party.id).unwrap().unwrap().display_name,
        "Person A"
    );
    assert_eq!(
        list_documents(&co).unwrap()[0].party_id.as_ref(),
        Some(&party.id)
    );
}

#[tokio::test]
async fn confirm_anonymizes_and_strips_docs_keeps_journal() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let co = dir.path().join("co");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = crm_upsert(
        &co,
        None,
        "Person B".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    attach_document(
        &co,
        DocumentKind::Other,
        "b.pdf".into(),
        Some(party.id.clone()),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let company = open_existing(&co).await.unwrap();
    let currency = Currency::new("DKK").unwrap();
    let amount = MinorAmount::from_minor(100);
    company
        .post(JournalEntry {
            as_of: chrono::Utc::now(),
            memo: "expense #receipt".into(),
            actor: owner,
            legs: vec![
                Leg {
                    account: "3000".into(),
                    direction: Direction::Debit,
                    amount,
                    currency: currency.clone(),
                    party_id: Some(party.id.clone()),
                },
                Leg {
                    account: "2000".into(),
                    direction: Direction::Credit,
                    amount,
                    currency,
                    party_id: None,
                },
            ],
        })
        .await
        .unwrap();

    let report = erase_party(
        &co,
        &party.id,
        ErasePartyOptions {
            confirm: true,
            delete_documents: false,
        },
    )
    .await
    .unwrap();
    assert!(!report.dry_run);
    assert_eq!(report.display_name_after, "erased");
    assert_eq!(report.documents_stripped.len(), 1);
    assert_eq!(report.journal_refs_retained.len(), 1);
    assert_eq!(
        get_party(&co, &party.id).unwrap().unwrap().display_name,
        "erased"
    );
    assert!(list_documents(&co).unwrap()[0].party_id.is_none());
}

#[tokio::test]
async fn confirm_delete_documents_uses_object_delete() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let co = dir.path().join("co");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = crm_upsert(
        &co,
        None,
        "Person C".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    attach_document(
        &co,
        DocumentKind::Receipt,
        "c.pdf".into(),
        Some(party.id.clone()),
        None,
        None,
        Some(b"bytes"),
    )
    .await
    .unwrap();
    let report = erase_party(
        &co,
        &party.id,
        ErasePartyOptions {
            confirm: true,
            delete_documents: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(report.documents_deleted.len(), 1);
    assert!(report.documents_stripped.is_empty());
    assert!(list_documents(&co).unwrap().is_empty());
    assert!(!co.join("c.pdf").exists());
    assert_eq!(
        get_party(&co, &party.id).unwrap().unwrap().display_name,
        "erased"
    );
}

#[tokio::test]
async fn confirm_appends_erase_audit_jsonl_no_secrets() {
    use crate::erase_audit::{erase_audit_path, EraseAuditLine, EraseAuditMode};
    use std::fs;

    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let co = dir.path().join("co");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = crm_upsert(
        &co,
        None,
        "Secret Name".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    attach_document(
        &co,
        DocumentKind::Receipt,
        "secret.pdf".into(),
        Some(party.id.clone()),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    erase_party(
        &co,
        &party.id,
        ErasePartyOptions {
            confirm: true,
            delete_documents: false,
        },
    )
    .await
    .unwrap();

    let path = erase_audit_path(&co);
    assert!(path.exists());
    let text = fs::read_to_string(&path).unwrap();
    let line: EraseAuditLine = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(line.party_id, party.id.to_string());
    assert_eq!(line.mode, EraseAuditMode::Confirm);
    assert_eq!(line.docs_touched, 1);
    assert!(line.unix_ms > 0);
    assert!(!text.contains("Secret Name"));
    assert!(!text.contains("secret.pdf"));
    assert!(!text.contains("display_name"));
}

#[tokio::test]
async fn dry_run_also_records_audit_mode() {
    use crate::erase_audit::{erase_audit_path, EraseAuditLine, EraseAuditMode};
    use std::fs;

    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let co = dir.path().join("co");
    init_company(&co, "Demo", &owner).await.unwrap();
    let party = crm_upsert(
        &co,
        None,
        "Preview".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
    )
    .unwrap();
    erase_party(&co, &party.id, ErasePartyOptions::default())
        .await
        .unwrap();
    let text = fs::read_to_string(erase_audit_path(&co)).unwrap();
    let line: EraseAuditLine = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(line.mode, EraseAuditMode::DryRun);
    assert_eq!(line.docs_touched, 0);
    assert!(!text.contains("Preview"));
}
