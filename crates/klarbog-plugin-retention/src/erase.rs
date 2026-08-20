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
#[path = "erase_tests.rs"]
mod erase_tests;
