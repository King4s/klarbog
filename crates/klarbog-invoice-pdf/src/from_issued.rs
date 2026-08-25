//! Build PDF payload from Klarbog issued-invoice JSON snapshot.

use crate::money_fmt::parse_amount_str_to_minor;
use crate::payload::{IssuedInvoicePdfPayload, PdfLine, PdfParty, PdfTotals};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssuedSnapshot {
    #[serde(default)]
    invoice_number: Option<String>,
    #[serde(default)]
    party_name: Option<String>,
    #[serde(default)]
    issue_date: Option<String>,
    #[serde(default)]
    due_date: Option<String>,
    #[serde(default)]
    lines: Vec<IssuedLine>,
    #[serde(default)]
    totals: Option<IssuedTotals>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssuedLine {
    #[serde(default)]
    description: String,
    #[serde(default)]
    amount_minor: Option<i64>,
    #[serde(default)]
    currency: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssuedTotals {
    #[serde(default)]
    net_amount: Option<String>,
    #[serde(default)]
    vat_amount: Option<String>,
    #[serde(default)]
    gross_amount: Option<String>,
    #[serde(default)]
    fx_rate_to_dkk_micro: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    net_amount_dkk_minor: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    vat_amount_dkk_minor: Option<i64>,
    #[serde(default)]
    gross_amount_dkk_minor: Option<i64>,
}

/// Map issued-invoice JSON bytes to a PDF payload. Unknown shapes → `None`.
pub fn payload_from_issued_json(bytes: &[u8]) -> Option<IssuedInvoicePdfPayload> {
    let snap: IssuedSnapshot = serde_json::from_slice(bytes).ok()?;
    let currency = snap
        .lines
        .first()
        .and_then(|l| l.currency.clone())
        .unwrap_or_else(|| "DKK".into());
    let lines: Vec<PdfLine> = snap
        .lines
        .into_iter()
        .map(|l| PdfLine {
            description: l.description,
            amount_minor: l.amount_minor,
            line_total_minor: l.amount_minor,
            ..Default::default()
        })
        .collect();
    let totals = snap.totals.map(|t| PdfTotals {
        net_minor: t.net_amount.as_deref().and_then(parse_amount_str_to_minor),
        vat_minor: t.vat_amount.as_deref().and_then(parse_amount_str_to_minor),
        gross_minor: t
            .gross_amount
            .as_deref()
            .and_then(parse_amount_str_to_minor),
        fx_rate_to_dkk_micro: t.fx_rate_to_dkk_micro,
        gross_amount_dkk_minor: t.gross_amount_dkk_minor,
        ..Default::default()
    });
    Some(IssuedInvoicePdfPayload {
        invoice_number: snap.invoice_number,
        issue_date: snap.issue_date,
        due_date: snap.due_date,
        currency: Some(currency),
        buyer: Some(PdfParty {
            name: snap.party_name,
            ..Default::default()
        }),
        lines,
        totals,
        ..Default::default()
    })
}

#[allow(clippy::too_many_arguments)]
pub fn payload_from_fields(
    invoice_number: &str,
    issue_date: Option<&str>,
    due_date: Option<&str>,
    buyer_name: Option<&str>,
    seller_name: Option<&str>,
    currency: &str,
    lines: &[(String, i64)],
    net_minor: i64,
    vat_minor: i64,
    gross_minor: i64,
    vat_rate_bps: Option<i64>,
    fx_rate_to_dkk_micro: Option<i64>,
    gross_amount_dkk_minor: Option<i64>,
) -> IssuedInvoicePdfPayload {
    IssuedInvoicePdfPayload {
        invoice_number: Some(invoice_number.to_string()),
        issue_date: issue_date.map(str::to_string),
        due_date: due_date.map(str::to_string),
        currency: Some(currency.to_string()),
        seller: seller_name.map(|n| PdfParty {
            name: Some(n.to_string()),
            ..Default::default()
        }),
        buyer: buyer_name.map(|n| PdfParty {
            name: Some(n.to_string()),
            ..Default::default()
        }),
        lines: lines
            .iter()
            .map(|(desc, minor)| PdfLine {
                description: desc.clone(),
                amount_minor: Some(*minor),
                line_total_minor: Some(*minor),
                ..Default::default()
            })
            .collect(),
        totals: Some(PdfTotals {
            net_minor: Some(net_minor),
            vat_minor: Some(vat_minor),
            gross_minor: Some(gross_minor),
            vat_rate_bps,
            fx_rate_to_dkk_micro,
            gross_amount_dkk_minor,
        }),
        ..Default::default()
    }
}
