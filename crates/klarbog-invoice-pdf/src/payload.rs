//! Issued-invoice PDF payload — money is i64 minor units (øre).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfParty {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vat_or_cvr: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfLine {
    #[serde(default)]
    pub description: String,
    /// Quantity in milli-units (1000 = 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity_milli: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_price_minor: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_total_minor: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax_classification: Option<String>,
    /// Klarbog issued-snapshot line amount (øre).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_minor: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfTotals {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_minor: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vat_minor: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gross_minor: Option<i64>,
    /// Basis points (2500 = 25 %).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vat_rate_bps: Option<i64>,
    /// FX rate × 1_000_000 (e.g. 7462300 → 7.462300).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_rate_to_dkk_micro: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gross_amount_dkk_minor: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPayment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_no: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_no: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iban: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_no: Option<String>,
}

/// Input to [`crate::build_issued_invoice_pdf`]. Same inputs → identical bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedInvoicePdfPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_period_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_period_end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seller: Option<PdfParty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buyer: Option<PdfParty>,
    #[serde(default)]
    pub lines: Vec<PdfLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totals: Option<PdfTotals>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment: Option<PdfPayment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse_charge_note: Option<String>,
}
