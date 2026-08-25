//! Deterministic issued-invoice PDF renderer (PDF 1.4 / Helvetica / WinAnsi).
//!
//! Same [`IssuedInvoicePdfPayload`] → identical bytes (no wall-clock in output).
//! Geometry uses integer centipoints; money uses i64 minor units (øre).

mod draw;
mod encode;
mod from_issued;
mod layout;
mod metrics;
mod money_fmt;
mod payload;
mod serialize;

pub use from_issued::{payload_from_fields, payload_from_issued_json};
pub use payload::{IssuedInvoicePdfPayload, PdfLine, PdfParty, PdfPayment, PdfTotals};
pub use serialize::build_issued_invoice_pdf;

use sha2::{Digest, Sha256};

/// Hex SHA-256 of PDF bytes.
pub fn pdf_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Object-store path hint for an issued invoice PDF snapshot.
pub fn issued_pdf_path_hint(invoice_no: &str) -> String {
    format!("invoices/issued/{invoice_no}.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{PdfLine, PdfParty, PdfTotals};

    fn sample_payload() -> IssuedInvoicePdfPayload {
        IssuedInvoicePdfPayload {
            invoice_number: Some("2026-0001".into()),
            issue_date: Some("2026-05-16".into()),
            due_date: Some("2026-06-15".into()),
            currency: Some("DKK".into()),
            seller: Some(PdfParty {
                name: Some("Sælger ApS".into()),
                vat_or_cvr: Some("12345678".into()),
                ..Default::default()
            }),
            buyer: Some(PdfParty {
                name: Some("Køber A/S".into()),
                ..Default::default()
            }),
            lines: vec![PdfLine {
                description: "Widget".into(),
                amount_minor: Some(12_500),
                line_total_minor: Some(12_500),
                ..Default::default()
            }],
            totals: Some(PdfTotals {
                net_minor: Some(10_000),
                vat_minor: Some(2_500),
                gross_minor: Some(12_500),
                vat_rate_bps: Some(2500),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn pdf_starts_with_magic() {
        let bytes = build_issued_invoice_pdf(&sample_payload());
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.windows(5).any(|w| w == b"%%EOF"));
    }

    #[test]
    fn deterministic_same_payload_same_hash() {
        let a = build_issued_invoice_pdf(&sample_payload());
        let b = build_issued_invoice_pdf(&sample_payload());
        assert_eq!(a, b);
        assert_eq!(pdf_sha256(&a), pdf_sha256(&b));
    }

    #[test]
    fn from_issued_json_roundtrip() {
        let json = br#"{
            "type":"issued_invoice",
            "invoiceNumber":"2026-0001",
            "partyName":"Buyer",
            "issueDate":"2026-05-16",
            "dueDate":"2026-06-15",
            "lines":[{"description":"Widget","amountMinor":12500,"currency":"DKK"}],
            "totals":{"netAmount":"100.00","vatAmount":"25.00","grossAmount":"125.00"}
        }"#;
        let payload = payload_from_issued_json(json).expect("parse");
        let pdf = build_issued_invoice_pdf(&payload);
        assert!(pdf.starts_with(b"%PDF"));
    }
}
