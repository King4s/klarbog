//! Resolve issued-invoice PDF attachment bytes for email delivery.

use crate::{Invoice, InvoiceError};
use klarbog_invoice_pdf::{
    build_issued_invoice_pdf, issued_pdf_path_hint, payload_from_fields, payload_from_issued_json,
};
use klarbog_storage::company_objects_root;
use std::fs;
use std::path::Path;

fn load_object(company: &Path, path_hint: &str) -> Result<Option<Vec<u8>>, InvoiceError> {
    let object_path = company_objects_root(company).join(path_hint);
    if !object_path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read(&object_path).map_err(InvoiceError::Io)?))
}

fn pdf_from_invoice(invoice: &Invoice, invoice_no: &str, buyer_name: Option<&str>) -> Vec<u8> {
    let (net, vat, gross, rate) = match &invoice.vat {
        Some(v) => (v.net_minor, v.vat_minor, v.gross_minor, Some(v.rate_bps)),
        None => {
            let total = invoice.total_minor().unwrap_or(0);
            (total, 0, total, None)
        }
    };
    let currency = invoice
        .lines
        .first()
        .map(|l| l.currency.as_str())
        .unwrap_or("DKK");
    let lines: Vec<(String, i64)> = invoice
        .lines
        .iter()
        .map(|l| (l.description.clone(), l.amount_minor))
        .collect();
    let fx = crate::fx::fx_totals_for_invoice(invoice).ok().flatten();
    let payload = payload_from_fields(
        invoice_no,
        invoice.issue_date.as_deref(),
        invoice.due_date.as_deref(),
        buyer_name,
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
    build_issued_invoice_pdf(&payload)
}

/// Prefer stored PDF snapshot; else render from issued JSON or invoice fields.
pub fn resolve_invoice_pdf_bytes(
    company: &Path,
    invoice: &Invoice,
    invoice_no: &str,
    buyer_name: Option<&str>,
) -> Result<Vec<u8>, InvoiceError> {
    let pdf_hint = issued_pdf_path_hint(invoice_no);
    if let Some(bytes) = load_object(company, &pdf_hint)? {
        if bytes.starts_with(b"%PDF") {
            return Ok(bytes);
        }
    }

    let json_hint = format!("invoices/issued/{invoice_no}.json");
    if let Some(json_bytes) = load_object(company, &json_hint)? {
        if let Some(mut payload) = payload_from_issued_json(&json_bytes) {
            // Thin fixtures may lack lines — fall back to live invoice lines.
            if payload.lines.is_empty() && !invoice.lines.is_empty() {
                return Ok(pdf_from_invoice(invoice, invoice_no, buyer_name));
            }
            if payload.invoice_number.is_none() {
                payload.invoice_number = Some(invoice_no.to_string());
            }
            return Ok(build_issued_invoice_pdf(&payload));
        }
    }

    Ok(pdf_from_invoice(invoice, invoice_no, buyer_name))
}
