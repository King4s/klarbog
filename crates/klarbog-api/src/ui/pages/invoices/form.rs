//! Invoice form types and status helpers.

use klarbog_plugin_invoice::InvoiceStatus;
use serde::Deserialize;

pub(super) struct InvoiceRow {
    pub id: String,
    pub status: String,
    pub total: String,
    pub party_id: String,
    pub can_send: bool,
    pub can_collect: bool,
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
