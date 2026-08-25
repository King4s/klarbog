//! Invoice form types and status helpers.

use klarbog_plugin_invoice::InvoiceStatus;
use serde::Deserialize;

pub(super) struct InvoiceRow {
    pub id: String,
    pub status: String,
    pub due: String,
    pub total: String,
    /// "—" for legacy invoices without frozen VAT (ADR-020).
    pub moms: String,
    pub brutto: String,
    pub party_id: String,
    pub can_send: bool,
    pub can_collect: bool,
    /// Sent and unpaid — eligible for a full credit note (ADR-020).
    pub can_credit: bool,
    /// Overdue collectible — morarente kan registreres/bogføres.
    pub can_interest: bool,
    pub has_unposted_interest: bool,
    /// Overdue commercial collectible — fast kompensation kan registreres/bogføres.
    pub can_compensate: bool,
    pub has_unposted_compensation: bool,
    /// Overdue collectible — rykkergebyr kan registreres/bogføres.
    pub can_reminder: bool,
    pub has_unposted_reminder: bool,
    /// Sent issued invoice — eligible for email delivery.
    pub can_email: bool,
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
    #[serde(default)]
    pub credit_amount_minor: Option<String>,
    /// Optional manual CN number (originalens creditNoteNumber).
    #[serde(default)]
    pub credit_note_number: Option<String>,
    /// Valgfrit manuelt fakturanummer ved send (originalens invoiceNumber).
    #[serde(default)]
    pub invoice_number: Option<String>,
    /// Valgfri forfaldsdato på kladde (YYYY-MM-DD).
    #[serde(default)]
    pub due_date: Option<String>,
    /// Morarente: pr. dato (YYYY-MM-DD), default i dag.
    #[serde(default)]
    pub as_of_date: Option<String>,
    /// Referencesats i bps (220 = 2,2 %); tom = lovlig tabel.
    #[serde(default)]
    pub reference_rate_bps: Option<String>,
    #[serde(default)]
    pub interest_note: Option<String>,
    /// Fast kompensation: beløb i øre; tom = lovligt maks (31000 = 310 DKK).
    #[serde(default)]
    pub compensation_amount_minor: Option<String>,
    #[serde(default)]
    pub compensation_note: Option<String>,
    /// Rykker: dato (YYYY-MM-DD), default i dag.
    #[serde(default)]
    pub reminder_date: Option<String>,
    /// Rykkergebyr i øre; tom = lovligt maks (10000 = 100 DKK).
    #[serde(default)]
    pub reminder_fee_minor: Option<String>,
    #[serde(default)]
    pub reminder_note: Option<String>,
    /// Optional recipient override for invoice email (DK-EMAIL-DELIVERY-001).
    #[serde(default)]
    pub email_to: Option<String>,
    /// `invoice` (default) or `reminder` for send_email (DK-EMAIL-DELIVERY-001).
    #[serde(default)]
    pub email_kind: Option<String>,
    /// When set, compound send_reminder skips fee booking (fee still registered).
    #[serde(default)]
    pub reminder_skip_book: Option<String>,
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
