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

pub async fn attach_document(
    company: &Path,
    kind: DocumentKind,
    path_hint: String,
    party_id: Option<PartyId>,
    invoice_id: Option<InvoiceId>,
    notes: Option<String>,
    content: Option<&[u8]>,
) -> Result<Document, DocumentError> {
    validate_path_hint(&path_hint)?;
    let storage = klarbog_storage(company)?;
    if let Some(ref pid) = party_id {
        get_party(company, pid)?.ok_or_else(|| DocumentError::PartyNotFound(pid.to_string()))?;
    }
    if let Some(ref iid) = invoice_id {
        get_invoice(company, iid)?
            .ok_or_else(|| DocumentError::InvoiceNotFound(iid.to_string()))?;
    }
    if let Some(bytes) = content {
        storage.put(&path_hint, bytes).await?;
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

pub async fn remove_document(
    company: &Path,
    id: &DocumentId,
    delete_object: bool,
) -> Result<Document, DocumentError> {
    let mut file = load_documents(company)?;
    let idx = file
        .documents
        .iter()
        .position(|d| d.id == *id)
        .ok_or_else(|| DocumentError::NotFound(id.to_string()))?;
    let removed = file.documents.remove(idx);
    if delete_object {
        let storage = klarbog_storage(company)?;
        match storage.delete(&removed.path_hint).await {
            Ok(()) => {}
            Err(klarbog_storage::StorageError::NotFound(_)) => {}
            Err(err) => return Err(err.into()),
        }
    }
    save_documents(company, &file)?;
    Ok(removed)
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
        closed_unix_ms: None,
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
    if open {
        exc.closed_unix_ms = None;
    } else {
        exc.closed_unix_ms = Some(chrono::Utc::now().timestamp_millis());
    }
    let updated = exc.clone();
    save_exceptions(company, &file)?;
    Ok(updated)
}

pub fn purge_closed_exceptions(
    company: &Path,
    before_unix_ms: i64,
    dry_run: bool,
) -> Result<Vec<ExceptionId>, DocumentError> {
    let mut file = load_exceptions(company)?;
    let to_remove: Vec<ExceptionId> = file
        .exceptions
        .iter()
        .filter(|e| {
            !e.open
                && e.closed_unix_ms
                    .is_some_and(|closed| closed < before_unix_ms)
        })
        .map(|e| e.id.clone())
        .collect();
    if !dry_run && !to_remove.is_empty() {
        file.exceptions
            .retain(|e| !to_remove.iter().any(|id| id == &e.id));
        save_exceptions(company, &file)?;
    }
    Ok(to_remove)
}

pub fn find_orphan_documents(company: &Path) -> Result<Vec<DocumentId>, DocumentError> {
    let mut orphans = Vec::new();
    for doc in list_documents(company)? {
        let mut orphan = false;
        if let Some(ref pid) = doc.party_id {
            if get_party(company, pid)?.is_none() {
                orphan = true;
            }
        }
        if !orphan {
            if let Some(ref iid) = doc.invoice_id {
                if get_invoice(company, iid)?.is_none() {
                    orphan = true;
                }
            }
        }
        if orphan {
            orphans.push(doc.id);
        }
    }
    Ok(orphans)
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod store_tests;
