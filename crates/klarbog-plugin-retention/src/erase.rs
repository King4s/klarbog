//! GDPR party erasure — anonymize CRM metadata; never mutate confirmed journal.

use crate::erase_audit::{append_erase_audit, EraseAuditLine, EraseAuditMode};
use crate::GdprError;
use klarbog_plugin_crm::{get_party, upsert_party};
use klarbog_plugin_documents::{
    document_ids_for_party, remove_document, strip_party_id_from_documents,
};
use klarbog_store_sqlite::open_company;
use klarbog_types::PartyId;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const ERASED_DISPLAY_NAME: &str = "erased";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ErasePartyOptions {
    /// Apply changes; default is dry-run preview only.
    pub confirm: bool,
    /// Delete documents referencing the party (object store delete); else strip `party_id`.
    pub delete_documents: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasePartyReport {
    pub dry_run: bool,
    pub party_id: String,
    pub display_name_before: String,
    pub display_name_after: String,
    pub documents_stripped: Vec<String>,
    pub documents_deleted: Vec<String>,
    /// Confirmed journal entry ids that still reference the party (immutable).
    pub journal_refs_retained: Vec<String>,
}

pub async fn erase_party(
    company: &Path,
    party_id: &PartyId,
    opts: ErasePartyOptions,
) -> Result<ErasePartyReport, GdprError> {
    let party = get_party(company, party_id)?
        .ok_or_else(|| GdprError::PartyNotFound(party_id.to_string()))?;
    let dry_run = !opts.confirm;
    let store = open_company(company).await?;
    let journal_refs_retained = store.list_entry_ids_for_party(party_id.as_str()).await?;

    let mut documents_stripped = Vec::new();
    let mut documents_deleted = Vec::new();

    if opts.delete_documents {
        let ids = document_ids_for_party(company, party_id)?;
        if dry_run {
            documents_deleted = ids.into_iter().map(|id| id.to_string()).collect();
        } else {
            for id in ids {
                let removed = remove_document(company, &id, true).await?;
                documents_deleted.push(removed.id.to_string());
            }
        }
    } else {
        let ids = strip_party_id_from_documents(company, party_id, dry_run)?;
        documents_stripped = ids.into_iter().map(|id| id.to_string()).collect();
    }

    if !dry_run {
        upsert_party(company, Some(party_id.clone()), ERASED_DISPLAY_NAME.into())?;
    }

    let docs_touched = documents_stripped.len() + documents_deleted.len();
    // Audit both dry_run and confirm (mode field); no display names or paths.
    append_erase_audit(
        company,
        &EraseAuditLine {
            party_id: party_id.to_string(),
            unix_ms: chrono::Utc::now().timestamp_millis(),
            mode: if dry_run {
                EraseAuditMode::DryRun
            } else {
                EraseAuditMode::Confirm
            },
            docs_touched,
        },
    )?;

    Ok(ErasePartyReport {
        dry_run,
        party_id: party_id.to_string(),
        display_name_before: party.display_name,
        display_name_after: ERASED_DISPLAY_NAME.into(),
        documents_stripped,
        documents_deleted,
        journal_refs_retained,
    })
}

#[cfg(test)]
mod tests {
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
        let party = crm_upsert(&co, None, "Person A".into()).unwrap();
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
        let party = crm_upsert(&co, None, "Person B".into()).unwrap();
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
                        account: "6000".into(),
                        direction: Direction::Debit,
                        amount,
                        currency: currency.clone(),
                        party_id: Some(party.id.clone()),
                    },
                    Leg {
                        account: "5800".into(),
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
        let party = crm_upsert(&co, None, "Person C".into()).unwrap();
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
        let party = crm_upsert(&co, None, "Secret Name".into()).unwrap();
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
        let party = crm_upsert(&co, None, "Preview".into()).unwrap();
        erase_party(&co, &party.id, ErasePartyOptions::default())
            .await
            .unwrap();
        let text = fs::read_to_string(erase_audit_path(&co)).unwrap();
        let line: EraseAuditLine = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(line.mode, EraseAuditMode::DryRun);
        assert_eq!(line.docs_touched, 0);
        assert!(!text.contains("Preview"));
    }
}
