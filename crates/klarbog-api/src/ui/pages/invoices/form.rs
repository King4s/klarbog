//! Invoice form types and status helpers.

use klarbog_plugin_invoice::InvoiceStatus;
use serde::Deserialize;

pub(super) struct InvoiceRow {
    pub id: String,
    pub status: String,
    pub total: String,
    /// "—" for legacy invoices without frozen VAT (ADR-020).
    pub moms: String,
    pub brutto: String,
    pub party_id: String,
    pub can_send: bool,
    pub can_collect: bool,
    /// Sent and unpaid — eligible for a full credit note (ADR-020).
    pub can_credit: bool,
}

pub(super) struct PartyOption {
    pub id: String,
    pub label: String,
}

#[derive(Deserialize)]
pub struct InvoiceActionForm {
    pub action: String,
    pub party_id: Option<String>,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub amount_minor: Option<String>,
    pub invoice_id: Option<String>,
    pub part_amount_minor: Option<String>,
    pub entry_json: Option<String>,
    pub confirm_token: Option<String>,
    /// Credit note reason — required, as in the original (DK-CREDIT-NOTE-001).
    pub credit_reason: Option<String>,
}

pub(super) fn status_label(s: InvoiceStatus) -> &'static str {
    match s {
        InvoiceStatus::Draft => "draft",
        InvoiceStatus::Sent => "sent",
        InvoiceStatus::PartPaid => "part_paid",
        InvoiceStatus::Paid => "paid",
        InvoiceStatus::Void => "void",
    }
}
