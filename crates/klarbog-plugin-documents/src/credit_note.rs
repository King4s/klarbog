//! Immutable credit-note JSON documents (DK-CREDIT-NOTE-001).
//! Mirrors original `credit-notes.ts`: canonical JSON payload, sha256, object
//! store under `invoices/issued/{CN}.json`, metadata in `documents.json`.

use crate::store::append_document;
use crate::{Document, DocumentError, DocumentId, DocumentKind};
use klarbog_plugin_invoice::{get_invoice, InvoiceId};
use klarbog_storage::klarbog_storage;
use klarbog_types::{load_fiscal_settings, retain_until_iso, PartyId};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreditNotePayload {
    #[serde(rename = "type")]
    kind: &'static str,
    credit_note_number: String,
    original_invoice_id: String,
    issue_date: String,
    reason: String,
    gross_amount: String,
    vat_amount: String,
    net_amount: String,
    credited_so_far: String,
    remaining_after_this_credit: String,
    issued_at: String,
}

fn format_dkk_minor(minor: i64) -> String {
    format!("{}.{:02}", minor / 100, minor.rem_euclid(100))
}

fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn credit_note_path_hint(credit_note_no: &str) -> String {
    format!("invoices/issued/{credit_note_no}.json")
}

/// Persist an immutable credit-note snapshot. Fail-closed if the CN path is
/// already registered (no overwrite).
#[allow(clippy::too_many_arguments)]
pub async fn attach_credit_note(
    company: &Path,
    credit_note_no: &str,
    invoice_id: &InvoiceId,
    party_id: &PartyId,
    issue_date: &str,
    reason: &str,
    net_minor: i64,
    vat_minor: i64,
    gross_minor: i64,
    credited_gross_minor_so_far: i64,
    remaining_gross_minor_after: i64,
    issued_at: chrono::DateTime<chrono::Utc>,
) -> Result<Document, DocumentError> {
    let invoice = get_invoice(company, invoice_id)?
        .ok_or_else(|| DocumentError::InvoiceNotFound(invoice_id.to_string()))?;
    if invoice.party_id != *party_id {
        return Err(DocumentError::PartyNotFound(party_id.to_string()));
    }

    let path_hint = credit_note_path_hint(credit_note_no);
    if crate::list_documents(company)?
        .iter()
        .any(|d| d.path_hint == path_hint)
    {
        return Err(DocumentError::CreditNoteExists(credit_note_no.to_string()));
    }

    let payload = CreditNotePayload {
        kind: "credit_note",
        credit_note_number: credit_note_no.to_string(),
        original_invoice_id: invoice_id.to_string(),
        issue_date: issue_date.to_string(),
        reason: reason.trim().to_string(),
        gross_amount: format_dkk_minor(gross_minor),
        vat_amount: format_dkk_minor(vat_minor),
        net_amount: format_dkk_minor(net_minor),
        credited_so_far: format_dkk_minor(credited_gross_minor_so_far),
        remaining_after_this_credit: format_dkk_minor(remaining_gross_minor_after),
        issued_at: issued_at.to_rfc3339(),
    };
    let serialized = serde_json::to_string_pretty(&payload)?;
    let hash = sha256_bytes(serialized.as_bytes());

    let storage = klarbog_storage(company)?;
    storage.put(&path_hint, serialized.as_bytes()).await?;

    let basis = chrono::NaiveDate::parse_from_str(issue_date, "%Y-%m-%d")
        .map_err(|_| DocumentError::InvalidIssueDate(issue_date.to_string()))?;
    let doc = Document {
        id: DocumentId::generate(),
        kind: DocumentKind::CreditNote,
        path_hint,
        party_id: Some(party_id.clone()),
        invoice_id: Some(invoice_id.clone()),
        notes: format!("Credit note {credit_note_no} for {invoice_id}"),
        created_unix_ms: issued_at.timestamp_millis(),
        retain_until: Some(retain_until_iso(basis, load_fiscal_settings(company))),
        sha256: Some(hash),
    };
    append_document(company, doc.clone())?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use klarbog_plugin_crm::{upsert_party, PartyKind};
    use klarbog_plugin_invoice::{create_draft_from_new, InvoiceKind, NewLine};
    use tempfile::tempdir;

    #[tokio::test]
    async fn attach_writes_immutable_json_with_sha256() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Kunde".into(), PartyKind::Private, None).unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id.clone(),
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Work".into(),
                amount_minor: 12_500,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        let issued = chrono::Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0).unwrap();
        let doc = attach_credit_note(
            &co,
            "CN-2026-0001",
            &inv.id,
            &party.id,
            "2026-05-20",
            "fejl",
            10_000,
            2_500,
            12_500,
            0,
            0,
            issued,
        )
        .await
        .unwrap();
        assert_eq!(doc.kind, DocumentKind::CreditNote);
        assert_eq!(doc.retain_until.as_deref(), Some("2031-12-31"));
        let hash = doc.sha256.as_ref().unwrap();
        assert_eq!(hash.len(), 64);

        let object_path = co.join("objects").join(&doc.path_hint);
        let bytes = std::fs::read(object_path).unwrap();
        assert_eq!(sha256_bytes(&bytes), *hash);
        assert!(std::str::from_utf8(&bytes)
            .unwrap()
            .contains("CN-2026-0001"));

        let err = attach_credit_note(
            &co,
            "CN-2026-0001",
            &inv.id,
            &party.id,
            "2026-05-20",
            "dup",
            1_000,
            0,
            1_000,
            12_500,
            0,
            issued,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DocumentError::CreditNoteExists(_)));
    }
}
