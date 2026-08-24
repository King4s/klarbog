use super::*;

use crate::DocumentKind;
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
use tempfile::tempdir;

#[tokio::test]
async fn document_roundtrip() {
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
        None,
    )
    .await
    .unwrap();
    assert!(co.join(DOCUMENTS_FILENAME).exists());
    assert_eq!(list_documents(&co).unwrap().len(), 1);
    assert!(get_document(&co, &doc.id).unwrap().is_some());
}

#[tokio::test]
async fn attach_puts_bytes_under_objects() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let bytes = b"pdf-bytes";
    attach_document(
        &co,
        DocumentKind::Receipt,
        "attachments/r.pdf".into(),
        None,
        None,
        None,
        Some(bytes),
    )
    .await
    .unwrap();
    let stored = co.join("objects").join("attachments/r.pdf");
    assert!(stored.is_file());
    assert_eq!(fs::read(stored).unwrap(), bytes);
}

#[tokio::test]
async fn remove_deletes_metadata_and_object() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let doc = attach_document(
        &co,
        DocumentKind::Receipt,
        "attachments/r.pdf".into(),
        None,
        None,
        None,
        Some(b"bytes"),
    )
    .await
    .unwrap();
    let stored = co.join("objects").join("attachments/r.pdf");
    assert!(stored.is_file());
    remove_document(&co, &doc.id, true).await.unwrap();
    assert!(list_documents(&co).unwrap().is_empty());
    assert!(!stored.exists());
}

#[tokio::test]
async fn remove_keeps_object_when_disabled() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let doc = attach_document(
        &co,
        DocumentKind::Receipt,
        "attachments/keep.pdf".into(),
        None,
        None,
        None,
        Some(b"stay"),
    )
    .await
    .unwrap();
    remove_document(&co, &doc.id, false).await.unwrap();
    assert!(list_documents(&co).unwrap().is_empty());
    assert!(co.join("objects/attachments/keep.pdf").is_file());
}

#[tokio::test]
async fn rejects_bad_path_hint() {
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
        None,
    )
    .await
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
    let closed = get_exception(&co, &exc.id).unwrap().unwrap();
    assert!(closed.closed_unix_ms.is_some());
    assert!(list_exceptions(&co, true).unwrap().is_empty());
    assert_eq!(list_exceptions(&co, false).unwrap().len(), 1);
}

#[tokio::test]
async fn validates_party_and_invoice() {
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    fs::create_dir_all(&co).unwrap();
    let party = upsert_party(
        &co,
        None,
        "Vendor".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();
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
        None,
    )
    .await
    .unwrap();
}
