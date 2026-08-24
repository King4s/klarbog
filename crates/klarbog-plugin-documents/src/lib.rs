//! Documents plugin — metadata in `documents.json`, exceptions in `exceptions.json`.
//! No journal-write capability (ADR-004). Optional binary upload via ObjectStore (ADR-007).

mod credit_note;
mod issued_invoice;
mod store;

pub use credit_note::attach_credit_note;
pub use issued_invoice::attach_issued_invoice;
pub use store::{
    attach_document, document_ids_for_party, find_orphan_documents, get_document, get_exception,
    list_documents, list_exceptions, purge_closed_exceptions, raise_exception, remove_document,
    set_exception_open, strip_party_id_from_documents, DOCUMENTS_FILENAME, EXCEPTIONS_FILENAME,
};

use klarbog_plugin::{Capability, Plugin};
use klarbog_plugin_invoice::InvoiceId;
use klarbog_types::PartyId;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Receipt,
    InvoiceScan,
    CreditNote,
    IssuedInvoice,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentId(String);

impl DocumentId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn generate() -> Self {
        Self(format!("doc_{}", uuid::Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub kind: DocumentKind,
    pub path_hint: String,
    pub party_id: Option<PartyId>,
    pub invoice_id: Option<InvoiceId>,
    pub notes: String,
    pub created_unix_ms: i64,
    /// ISO date YYYY-MM-DD — fiscal year end + 5 years from basis date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_until: Option<String>,
    /// Content digest for immutable snapshots (credit notes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceptionSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExceptionId(String);

impl ExceptionId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn generate() -> Self {
        Self(format!("exc_{}", uuid::Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ExceptionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exception {
    pub id: ExceptionId,
    pub code: String,
    pub severity: ExceptionSeverity,
    pub message: String,
    pub related_ids: Vec<String>,
    pub open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_unix_ms: Option<i64>,
}

#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("document not found: {0}")]
    NotFound(String),
    #[error("exception not found: {0}")]
    ExceptionNotFound(String),
    #[error("party not found: {0}")]
    PartyNotFound(String),
    #[error("invoice not found: {0}")]
    InvoiceNotFound(String),
    #[error("exception code must not be empty")]
    EmptyCode,
    #[error("exception message must not be empty")]
    EmptyMessage,
    #[error("invalid path hint: {0}")]
    InvalidPathHint(String),
    #[error("invalid issue date: {0}")]
    InvalidIssueDate(String),
    #[error("credit note already exists: {0}")]
    CreditNoteExists(String),
    #[error("issued invoice already exists: {0}")]
    IssuedInvoiceExists(String),
    #[error(transparent)]
    Storage(#[from] klarbog_storage::StorageError),
    #[error(transparent)]
    Crm(#[from] klarbog_plugin_crm::CrmError),
    #[error(transparent)]
    Invoice(#[from] klarbog_plugin_invoice::InvoiceError),
}

pub struct DocumentsPlugin;

impl Default for DocumentsPlugin {
    fn default() -> Self {
        Self
    }
}

impl DocumentsPlugin {
    #[allow(clippy::too_many_arguments)]
    pub async fn attach(
        &self,
        company: &Path,
        kind: DocumentKind,
        path_hint: String,
        party_id: Option<PartyId>,
        invoice_id: Option<InvoiceId>,
        notes: Option<String>,
        content: Option<&[u8]>,
    ) -> Result<Document, DocumentError> {
        attach_document(
            company, kind, path_hint, party_id, invoice_id, notes, content,
        )
        .await
    }

    pub fn list(&self, company: &Path) -> Result<Vec<Document>, DocumentError> {
        list_documents(company)
    }

    pub fn get(&self, company: &Path, id: &DocumentId) -> Result<Option<Document>, DocumentError> {
        get_document(company, id)
    }

    pub async fn remove(
        &self,
        company: &Path,
        id: &DocumentId,
        delete_object: bool,
    ) -> Result<Document, DocumentError> {
        remove_document(company, id, delete_object).await
    }

    pub fn raise(
        &self,
        company: &Path,
        code: String,
        severity: ExceptionSeverity,
        message: String,
        related_ids: Vec<String>,
    ) -> Result<Exception, DocumentError> {
        raise_exception(company, code, severity, message, related_ids)
    }

    pub fn list_open(&self, company: &Path) -> Result<Vec<Exception>, DocumentError> {
        list_exceptions(company, true)
    }

    pub fn close(&self, company: &Path, id: &ExceptionId) -> Result<Exception, DocumentError> {
        set_exception_open(company, id, false)
    }
}

impl Plugin for DocumentsPlugin {
    fn id(&self) -> &'static str {
        "documents"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::Read]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn documents_has_no_journal_write() {
        let p = DocumentsPlugin;
        assert!(!p.has_journal_write());
    }

    #[tokio::test]
    async fn plugin_attach_list() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let plugin = DocumentsPlugin;
        plugin
            .attach(
                &co,
                DocumentKind::Receipt,
                "files/r.pdf".into(),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(plugin.list(&co).unwrap().len(), 1);
    }
}
