//! Shared invoice fixtures for unit tests (draft + credit).

use crate::{Invoice, InvoiceId, InvoiceKind, InvoiceLine, InvoiceStatus, InvoiceVat};
use klarbog_types::{Currency, PartyId};

pub(crate) fn sample_invoice(kind: InvoiceKind) -> Invoice {
    Invoice {
        id: InvoiceId::new("inv_test"),
        party_id: PartyId::new("party_acme"),
        kind,
        lines: vec![InvoiceLine {
            description: "Widget".into(),
            amount_minor: 25_000,
            currency: Currency::new("DKK").unwrap(),
        }],
        status: InvoiceStatus::Draft,
        payments: Vec::new(),
        credits: Vec::new(),
        vat: None,
        credit_note_no: None,
        issue_date: None,
        due_date: None,
        invoice_no: None,
        issued_document_id: None,
        issued_sha256: None,
        interest_claims: Vec::new(),
        reminders: Vec::new(),
        compensation_claims: Vec::new(),
        fx_rate_to_dkk_micro: None,
    }
}

pub(crate) fn vat_invoice(kind: InvoiceKind) -> Invoice {
    let mut inv = sample_invoice(kind);
    // 25000 net → 6250 vat → 31250 gross (business convention).
    inv.vat = Some(InvoiceVat {
        net_minor: 25_000,
        vat_minor: 6_250,
        gross_minor: 31_250,
        rate_bps: 2_500,
    });
    inv
}
