//! Immutable issued-invoice JSON + PDF snapshot (DK-INVOICE-ISSUE-001).

use crate::store::append_document;
use crate::{Document, DocumentError, DocumentId, DocumentKind};
use klarbog_invoice_pdf::{
    build_issued_invoice_pdf, issued_pdf_path_hint, payload_from_fields, pdf_sha256,
};
use klarbog_plugin_crm::get_party;
use klarbog_plugin_invoice::{
    due_date::{add_days, format_iso_date, parse_iso_date},
    fx_totals_for_invoice, get_invoice, Invoice, InvoiceId,
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
    /// Present only when FX conversion was recorded on the invoice.
    #[serde(skip_serializing_if = "Option::is_none")]
    fx_rate_to_dkk_micro: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_amount_dkk_minor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vat_amount_dkk_minor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gross_amount_dkk_minor: Option<i64>,
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
    /// Invoice document currency (from lines; product is DKK-first today).
    currency: String,
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
    let currency = invoice
        .lines
        .first()
        .map(|l| l.currency.as_str().to_string())
        .unwrap_or_else(|| "DKK".to_string());
    let fx = fx_totals_for_invoice(invoice).map_err(DocumentError::Invoice)?;
    Ok(IssuedInvoicePayload {
        kind: "issued_invoice",
        invoice_number: invoice_no.to_string(),
        invoice_id: invoice.id.to_string(),
        party_id: invoice.party_id.to_string(),
        party_name: String::new(), // filled by caller
        issue_date: invoice.issue_date.clone().unwrap_or_default(),
        due_date: invoice.due_date.clone(),
        currency,
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
            fx_rate_to_dkk_micro: fx.as_ref().map(|f| f.fx_rate_to_dkk_micro),
            net_amount_dkk_minor: fx.as_ref().map(|f| f.net_amount_dkk_minor),
            vat_amount_dkk_minor: fx.as_ref().map(|f| f.vat_amount_dkk_minor),
            gross_amount_dkk_minor: fx.as_ref().map(|f| f.gross_amount_dkk_minor),
        },
        issued_at: issued_at.to_string(),
    })
}

fn pdf_bytes_for_issued(
    invoice: &Invoice,
    invoice_no: &str,
    issue_date: &str,
    due_date: Option<&str>,
    party_name: &str,
) -> Result<Vec<u8>, DocumentError> {
    let (net, vat, gross, rate) = match &invoice.vat {
        Some(v) => (v.net_minor, v.vat_minor, v.gross_minor, Some(v.rate_bps)),
        None => {
            let total = invoice.total_minor().map_err(DocumentError::Invoice)?;
            (total, 0, total, None)
        }
    };
    let currency = invoice
        .lines
        .first()
        .map(|l| l.currency.as_str())
        .unwrap_or("DKK");
    let fx = fx_totals_for_invoice(invoice).map_err(DocumentError::Invoice)?;
    let lines: Vec<(String, i64)> = invoice
        .lines
        .iter()
        .map(|l| (l.description.clone(), l.amount_minor))
        .collect();
    let payload = payload_from_fields(
        invoice_no,
        Some(issue_date),
        due_date,
        Some(party_name),
        None,
        currency,
        &lines,
        net,
        vat,
        gross,
        rate,
        fx.as_ref().map(|f| f.fx_rate_to_dkk_micro),
        fx.as_ref().map(|f| f.gross_amount_dkk_minor),
    );
    Ok(build_issued_invoice_pdf(&payload))
}

/// Persist immutable issued-invoice snapshot (JSON + PDF). Fail-closed on duplicate.
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
    let pdf_hint = issued_pdf_path_hint(invoice_no);
    if crate::list_documents(company)?
        .iter()
        .any(|d| d.path_hint == path_hint || d.path_hint == pdf_hint)
    {
        return Err(DocumentError::IssuedInvoiceExists(invoice_no.to_string()));
    }

    let mut payload = payload_from_invoice(&invoice, invoice_no, &issued_at.to_rfc3339())?;
    payload.issue_date = issue_date.to_string();
    payload.due_date = due_date.clone();
    payload.party_name = party.display_name.clone();

    let serialized = serde_json::to_string_pretty(&payload)?;
    let hash = sha256_bytes(serialized.as_bytes());

    let pdf_bytes = pdf_bytes_for_issued(
        &invoice,
        invoice_no,
        issue_date,
        due_date.as_deref(),
        &party.display_name,
    )?;
    let pdf_hash = pdf_sha256(&pdf_bytes);

    let storage = klarbog_storage(company)?;
    storage.put(&path_hint, serialized.as_bytes()).await?;
    storage.put(&pdf_hint, &pdf_bytes).await?;

    let basis = chrono::NaiveDate::parse_from_str(issue_date, "%Y-%m-%d")
        .map_err(|_| DocumentError::InvalidIssueDate(issue_date.to_string()))?;
    let doc = Document {
        id: DocumentId::generate(),
        kind: DocumentKind::IssuedInvoice,
        path_hint,
        party_id: Some(invoice.party_id.clone()),
        invoice_id: Some(invoice_id.clone()),
        notes: format!("Issued invoice {invoice_no}; pdf={pdf_hint}; pdf_sha256={pdf_hash}"),
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
        let json = std::fs::read_to_string(co.join("objects").join(&doc.path_hint)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["currency"], "DKK");
        assert!(v["totals"].get("fxRateToDkkMicro").is_none());
    }

    #[tokio::test]
    async fn attach_writes_eur_snapshot_with_dkk_totals() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Buyer GmbH".into(),
            PartyKind::Private,
            None,
            None,
        )
        .unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Consulting".into(),
                amount_minor: 12_500,
                currency: "EUR".into(),
            }],
        )
        .unwrap();
        let inv_path = co.join("invoices.json");
        let mut file: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&inv_path).unwrap()).unwrap();
        file["invoices"][0]["vat"] = serde_json::json!({
            "net_minor": 10_000,
            "vat_minor": 2_500,
            "gross_minor": 12_500,
            "rate_bps": 2_500
        });
        file["invoices"][0]["fx_rate_to_dkk_micro"] = serde_json::json!(7_460_000);
        std::fs::write(
            &inv_path,
            serde_json::to_string_pretty(&file).expect("write invoices.json"),
        )
        .unwrap();
        let issued_at = chrono::Utc.with_ymd_and_hms(2026, 5, 16, 12, 0, 0).unwrap();
        attach_issued_invoice(&co, &inv.id, "2026-0001", "2026-05-16", 30, issued_at)
            .await
            .unwrap();
        let json =
            std::fs::read_to_string(co.join("objects/invoices/issued/2026-0001.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["currency"], "EUR");
        assert_eq!(v["totals"]["fxRateToDkkMicro"].as_i64(), Some(7_460_000));
        assert_eq!(v["totals"]["grossAmountDkkMinor"].as_i64(), Some(93_250));
    }

    #[tokio::test]
    async fn attach_writes_pdf_alongside_json() {
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
        let pdf_path = co.join("objects/invoices/issued/2026-0001.pdf");
        assert!(pdf_path.exists(), "pdf snapshot missing");
        let bytes = std::fs::read(&pdf_path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(doc.notes.contains("pdf_sha256="));
        assert!(doc.notes.contains("invoices/issued/2026-0001.pdf"));
    }
}
