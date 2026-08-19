//! Per-company `documents.json` + `exceptions.json` persistence (slice 7).

use crate::{
    Document, DocumentError, DocumentId, DocumentKind, Exception, ExceptionId, ExceptionSeverity,
};
use klarbog_plugin_crm::get_party;
use klarbog_plugin_invoice::{get_invoice, InvoiceId};
use klarbog_storage::klarbog_storage;
use klarbog_types::PartyId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const DOCUMENTS_FILENAME: &str = "documents.json";
pub const EXCEPTIONS_FILENAME: &str = "exceptions.json";

fn validate_path_hint(hint: &str) -> Result<(), DocumentError> {
    if hint.trim().is_empty() {
        return Err(DocumentError::InvalidPathHint(
            "path hint must not be empty".into(),
        ));
    }
    let path = Path::new(hint);
    if path.is_absolute() {
        return Err(DocumentError::InvalidPathHint(
            "path hint must be relative under company".into(),
        ));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(DocumentError::InvalidPathHint(
            "path hint contains '..'".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DocumentsFile {
    documents: Vec<Document>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExceptionsFile {
    exceptions: Vec<Exception>,
}

fn documents_path(company: &Path) -> PathBuf {
    company.join(DOCUMENTS_FILENAME)
}

fn exceptions_path(company: &Path) -> PathBuf {
    company.join(EXCEPTIONS_FILENAME)
}

fn load_documents(company: &Path) -> Result<DocumentsFile, DocumentError> {
    let path = documents_path(company);
    if !path.exists() {
        return Ok(DocumentsFile::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save_documents(company: &Path, file: &DocumentsFile) -> Result<(), DocumentError> {
    let json = serde_json::to_string_pretty(file)?;
    fs::write(documents_path(company), json)?;
    Ok(())
}

fn load_exceptions(company: &Path) -> Result<ExceptionsFile, DocumentError> {
    let path = exceptions_path(company);
    if !path.exists() {
        return Ok(ExceptionsFile::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save_exceptions(company: &Path, file: &ExceptionsFile) -> Result<(), DocumentError> {
    let json = serde_json::to_string_pretty(file)?;
    fs::write(exceptions_path(company), json)?;
    Ok(())
}

pub fn list_documents(company: &Path) -> Result<Vec<Document>, DocumentError> {
    Ok(load_documents(company)?.documents)
}

pub fn get_document(company: &Path, id: &DocumentId) -> Result<Option<Document>, DocumentError> {
    Ok(load_documents(company)?
        .documents
        .into_iter()
        .find(|d| d.id == *id))
}

pub fn attach_document(
    company: &Path,
    kind: DocumentKind,
    path_hint: String,
    party_id: Option<PartyId>,
    invoice_id: Option<InvoiceId>,
    notes: Option<String>,
) -> Result<Document, DocumentError> {
    validate_path_hint(&path_hint)?;
    klarbog_storage(company)?;
    if let Some(ref pid) = party_id {
        get_party(company, pid)?.ok_or_else(|| DocumentError::PartyNotFound(pid.to_string()))?;
    }
    if let Some(ref iid) = invoice_id {
        get_invoice(company, iid)?
            .ok_or_else(|| DocumentError::InvoiceNotFound(iid.to_string()))?;
    }
    let doc = Document {
        id: DocumentId::generate(),
        kind,
        path_hint,
        party_id,
        invoice_id,
        notes: notes.unwrap_or_default(),
        created_unix_ms: chrono::Utc::now().timestamp_millis(),
    };
    let mut file = load_documents(company)?;
    file.documents.push(doc.clone());
    save_documents(company, &file)?;
    Ok(doc)
}

pub fn list_exceptions(company: &Path, open_only: bool) -> Result<Vec<Exception>, DocumentError> {
    let items = load_exceptions(company)?.exceptions;
    Ok(if open_only {
        items.into_iter().filter(|e| e.open).collect()
    } else {
        items
    })
}

pub fn get_exception(company: &Path, id: &ExceptionId) -> Result<Option<Exception>, DocumentError> {
    Ok(load_exceptions(company)?
        .exceptions
        .into_iter()
        .find(|e| e.id == *id))
}

pub fn raise_exception(
    company: &Path,
    code: String,
    severity: ExceptionSeverity,
    message: String,
    related_ids: Vec<String>,
) -> Result<Exception, DocumentError> {
    if code.trim().is_empty() {
        return Err(DocumentError::EmptyCode);
    }
    if message.trim().is_empty() {
        return Err(DocumentError::EmptyMessage);
    }
    let exc = Exception {
        id: ExceptionId::generate(),
        code,
        severity,
        message,
        related_ids,
        open: true,
    };
    let mut file = load_exceptions(company)?;
    file.exceptions.push(exc.clone());
    save_exceptions(company, &file)?;
    Ok(exc)
}

pub fn set_exception_open(
    company: &Path,
    id: &ExceptionId,
    open: bool,
) -> Result<Exception, DocumentError> {
    let mut file = load_exceptions(company)?;
    let exc = file
        .exceptions
        .iter_mut()
        .find(|e| e.id == *id)
        .ok_or_else(|| DocumentError::ExceptionNotFound(id.to_string()))?;
    exc.open = open;
    let updated = exc.clone();
    save_exceptions(company, &file)?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DocumentKind;
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
    use tempfile::tempdir;

    #[test]
    fn document_roundtrip() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let doc = attach_document(
            &co,
            DocumentKind::Receipt,
            "attachments/r.pdf".into(),
            None,
            None,
            Some("note".into()),
        )
        .unwrap();
        assert!(co.join(DOCUMENTS_FILENAME).exists());
        assert_eq!(list_documents(&co).unwrap().len(), 1);
        assert!(get_document(&co, &doc.id).unwrap().is_some());
    }

    #[test]
    fn rejects_bad_path_hint() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let err = attach_document(
            &co,
            DocumentKind::Other,
            "../escape.pdf".into(),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, DocumentError::InvalidPathHint(_)));
    }

    #[test]
    fn exception_raise_and_close() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let exc = raise_exception(
            &co,
            "missing_attachment".into(),
            ExceptionSeverity::Warn,
            "No receipt".into(),
            vec!["inv_x".into()],
        )
        .unwrap();
        assert!(co.join(EXCEPTIONS_FILENAME).exists());
        assert_eq!(list_exceptions(&co, true).unwrap().len(), 1);
        set_exception_open(&co, &exc.id, false).unwrap();
        assert!(list_exceptions(&co, true).unwrap().is_empty());
        assert_eq!(list_exceptions(&co, false).unwrap().len(), 1);
    }

    #[test]
    fn validates_party_and_invoice() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Vendor".into()).unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id.clone(),
            InvoiceKind::Purchase,
            vec![NewLine {
                description: "Goods".into(),
                amount_minor: 1000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        attach_document(
            &co,
            DocumentKind::InvoiceScan,
            "scans/inv.pdf".into(),
            Some(party.id),
            Some(inv.id),
            None,
        )
        .unwrap();
    }
}
