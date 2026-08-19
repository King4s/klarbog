//! GDPR subject export stub — party ids + document path_hints only (slice 9, DEV).

use klarbog_plugin_crm::list_parties;
use klarbog_plugin_documents::list_documents;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const GDPR_EXPORT_FILENAME: &str = "gdpr_export.json";

#[derive(Debug, Error)]
pub enum GdprError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("crm: {0}")]
    Crm(#[from] klarbog_plugin_crm::CrmError),
    #[error("documents: {0}")]
    Documents(#[from] klarbog_plugin_documents::DocumentError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprDocumentHint {
    pub id: String,
    pub path_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprExport {
    pub exported_unix_ms: i64,
    pub party_ids: Vec<String>,
    pub documents: Vec<GdprDocumentHint>,
}

pub fn build_gdpr_export(company: &Path) -> Result<GdprExport, GdprError> {
    let party_ids: Vec<String> = list_parties(company)?
        .into_iter()
        .map(|p| p.id.to_string())
        .collect();
    let documents = list_documents(company)?
        .into_iter()
        .map(|d| GdprDocumentHint {
            id: d.id.to_string(),
            path_hint: d.path_hint,
        })
        .collect();
    Ok(GdprExport {
        exported_unix_ms: chrono::Utc::now().timestamp_millis(),
        party_ids,
        documents,
    })
}

pub fn write_gdpr_export(company: &Path) -> Result<GdprExport, GdprError> {
    let export = build_gdpr_export(company)?;
    let json = serde_json::to_string_pretty(&export)?;
    fs::write(company.join(GDPR_EXPORT_FILENAME), json)?;
    Ok(export)
}

pub fn gdpr_export_path(company: &Path) -> PathBuf {
    company.join(GDPR_EXPORT_FILENAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_documents::{attach_document, DocumentKind};
    use tempfile::tempdir;

    #[test]
    fn stub_has_ids_and_hints_only() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Test Person".into()).unwrap();
        attach_document(
            &co,
            DocumentKind::Receipt,
            "attachments/r.pdf".into(),
            Some(party.id),
            None,
            None,
        )
        .unwrap();
        let export = write_gdpr_export(&co).unwrap();
        assert_eq!(export.party_ids.len(), 1);
        assert_eq!(export.documents.len(), 1);
        assert_eq!(export.documents[0].path_hint, "attachments/r.pdf");
        assert!(gdpr_export_path(&co).exists());
    }
}
