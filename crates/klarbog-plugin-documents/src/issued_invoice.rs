//! Immutable issued-invoice JSON (DK-INVOICE-ISSUE-001).

use crate::store::append_document;
use crate::{Document, DocumentError, DocumentId, DocumentKind};
use klarbog_plugin_crm::get_party;
use klarbog_plugin_invoice::{
    due_date::{add_days, format_iso_date, parse_iso_date},
    get_invoice, Invoice, InvoiceId,
};
use klarbog_storage::klarbog_storage;
use klarbog_types::{load_fiscal_settings, retain_until_iso};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct IssuedLine {
    description: String,
    amount_minor: i64,
    currency: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct IssuedTotals {
    net_amount: String,
    vat_amount: String,
    gross_amount: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct IssuedInvoicePayload {
    #[serde(rename = "type")]
    kind: &'static str,
    invoice_number: String,
    invoice_id: String,
    party_id: String,
    party_name: String,
    issue_date: String,
    due_date: Option<String>,
    lines: Vec<IssuedLine>,
    totals: IssuedTotals,
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

fn issued_path_hint(invoice_no: &str) -> String {
    format!("invoices/issued/{invoice_no}.json")
}

fn payload_from_invoice(
    invoice: &Invoice,
    invoice_no: &str,
    issued_at: &str,
) -> Result<IssuedInvoicePayload, DocumentError> {
    let (net, vat, gross) = match &invoice.vat {
        Some(v) => (v.net_minor, v.vat_minor, v.gross_minor),
        None => {
            let total = invoice.total_minor().map_err(DocumentError::Invoice)?;
            (total, 0, total)
        }
    };
    Ok(IssuedInvoicePayload {
        kind: "issued_invoice",
        invoice_number: invoice_no.to_string(),
        invoice_id: invoice.id.to_string(),
        party_id: invoice.party_id.to_string(),
        party_name: String::new(), // filled by caller
        issue_date: invoice.issue_date.clone().unwrap_or_default(),
        due_date: invoice.due_date.clone(),
        lines: invoice
            .lines
            .iter()
            .map(|l| IssuedLine {
                description: l.description.clone(),
                amount_minor: l.amount_minor,
                currency: l.currency.as_str().to_string(),
            })
            .collect(),
        totals: IssuedTotals {
            net_amount: format_dkk_minor(net),
            vat_amount: format_dkk_minor(vat),
            gross_amount: format_dkk_minor(gross),
        },
        issued_at: issued_at.to_string(),
    })
}

/// Persist immutable issued-invoice snapshot. Fail-closed on duplicate path.
pub async fn attach_issued_invoice(
    company: &Path,
    invoice_id: &InvoiceId,
    invoice_no: &str,
    issue_date: &str,
    payment_terms_days: u32,
    issued_at: chrono::DateTime<chrono::Utc>,
) -> Result<Document, DocumentError> {
    let invoice = get_invoice(company, invoice_id)?
        .ok_or_else(|| DocumentError::InvoiceNotFound(invoice_id.to_string()))?;

    let party = get_party(company, &invoice.party_id)?
        .ok_or_else(|| DocumentError::PartyNotFound(invoice.party_id.to_string()))?;

    let due_date = if invoice
        .due_date
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        invoice.due_date.clone()
    } else {
        let issue = parse_iso_date(issue_date).map_err(DocumentError::Invoice)?;
        Some(format_iso_date(add_days(
            issue,
            i64::from(payment_terms_days),
        )))
    };

    let path_hint = issued_path_hint(invoice_no);
    if crate::list_documents(company)?
        .iter()
        .any(|d| d.path_hint == path_hint)
    {
        return Err(DocumentError::IssuedInvoiceExists(invoice_no.to_string()));
    }

    let mut payload = payload_from_invoice(&invoice, invoice_no, &issued_at.to_rfc3339())?;
    payload.issue_date = issue_date.to_string();
    payload.due_date = due_date;
    payload.party_name = party.display_name;

    let serialized = serde_json::to_string_pretty(&payload)?;
    let hash = sha256_bytes(serialized.as_bytes());

    let storage = klarbog_storage(company)?;
    storage.put(&path_hint, serialized.as_bytes()).await?;

    let basis = chrono::NaiveDate::parse_from_str(issue_date, "%Y-%m-%d")
        .map_err(|_| DocumentError::InvalidIssueDate(issue_date.to_string()))?;
    let doc = Document {
        id: DocumentId::generate(),
        kind: DocumentKind::IssuedInvoice,
        path_hint,
        party_id: Some(invoice.party_id.clone()),
        invoice_id: Some(invoice_id.clone()),
        notes: format!("Issued invoice {invoice_no}"),
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
    async fn attach_writes_immutable_json() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party =
            upsert_party(&co, None, "Buyer".into(), PartyKind::Private, None, None).unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Widget".into(),
                amount_minor: 12_500,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        let issued_at = chrono::Utc.with_ymd_and_hms(2026, 5, 16, 12, 0, 0).unwrap();
        let doc = attach_issued_invoice(&co, &inv.id, "2026-0001", "2026-05-16", 30, issued_at)
            .await
            .unwrap();
        assert!(doc.sha256.is_some());
        assert_eq!(doc.kind, DocumentKind::IssuedInvoice);
        assert!(co.join("objects").join(&doc.path_hint).exists());
    }
}
