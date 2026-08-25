//! Foreign-currency → DKK conversion for issued-invoice snapshots (no f64).

use crate::{Invoice, InvoiceError, InvoiceVat};
use klarbog_types::MinorAmount;

/// FX rate scale: rate × `FX_RATE_MICRO` (6 decimal places, mirrors TS `roundRate6`).
pub const FX_RATE_MICRO: i64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvoiceFxTotals {
    pub fx_rate_to_dkk_micro: i64,
    pub net_amount_dkk_minor: i64,
    pub vat_amount_dkk_minor: i64,
    pub gross_amount_dkk_minor: i64,
}

/// Convert foreign minor units to DKK minor at `rate_micro` (half-even rounding).
pub fn convert_minor_at_rate(foreign_minor: i64, rate_micro: i64) -> Result<i64, InvoiceError> {
    if rate_micro <= 0 {
        return Err(InvoiceError::InvalidFxRate);
    }
    let numer = i128::from(foreign_minor)
        .checked_mul(i128::from(rate_micro))
        .ok_or(InvoiceError::Overflow)?;
    MinorAmount::from_ratio_half_even(numer, i128::from(FX_RATE_MICRO))
        .map(|m| m.minor())
        .map_err(|_| InvoiceError::Overflow)
}

/// Build DKK totals for issued snapshot when invoice currency ≠ DKK.
pub fn fx_totals_for_invoice(invoice: &Invoice) -> Result<Option<InvoiceFxTotals>, InvoiceError> {
    invoice.validate_lines()?;
    let currency = invoice.lines[0].currency.as_str();
    if currency == "DKK" {
        return Ok(None);
    }
    let rate = invoice
        .fx_rate_to_dkk_micro
        .filter(|r| *r > 0)
        .ok_or_else(|| InvoiceError::MissingFxRate(currency.to_string()))?;
    let (net, vat, gross) = booking_amounts(invoice)?;
    let totals = InvoiceFxTotals {
        fx_rate_to_dkk_micro: rate,
        net_amount_dkk_minor: convert_minor_at_rate(net, rate)?,
        vat_amount_dkk_minor: convert_minor_at_rate(vat, rate)?,
        gross_amount_dkk_minor: convert_minor_at_rate(gross, rate)?,
    };
    Ok(Some(totals))
}

fn booking_amounts(invoice: &Invoice) -> Result<(i64, i64, i64), InvoiceError> {
    match &invoice.vat {
        Some(InvoiceVat {
            net_minor,
            vat_minor,
            gross_minor,
            ..
        }) => Ok((*net_minor, *vat_minor, *gross_minor)),
        None => {
            let total = invoice.total_minor()?;
            Ok((total, 0, total))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InvoiceKind, InvoiceLine};
    use klarbog_types::Currency;

    fn eur_invoice(net: i64, vat: i64, gross: i64, rate_micro: i64) -> Invoice {
        Invoice {
            id: crate::InvoiceId::new("inv_test"),
            party_id: klarbog_types::PartyId::new("pty"),
            kind: InvoiceKind::Sale,
            status: crate::InvoiceStatus::Draft,
            lines: vec![InvoiceLine {
                description: "Consulting".into(),
                amount_minor: gross,
                currency: Currency::new("EUR").unwrap(),
            }],
            vat: Some(crate::InvoiceVat {
                net_minor: net,
                vat_minor: vat,
                gross_minor: gross,
                rate_bps: 2_500,
            }),
            fx_rate_to_dkk_micro: Some(rate_micro),
            payments: vec![],
            credits: vec![],
            interest_claims: vec![],
            reminders: vec![],
            compensation_claims: vec![],
            credit_note_no: None,
            issue_date: None,
            due_date: None,
            invoice_no: None,
            issued_document_id: None,
            issued_sha256: None,
        }
    }

    #[test]
    fn converts_eur_totals_at_rate() {
        // 125 EUR gross × 7.46 = 932.50 DKK
        let rate = 7_460_000_i64;
        let inv = eur_invoice(10_000, 2_500, 12_500, rate);
        let fx = fx_totals_for_invoice(&inv).unwrap().expect("fx totals");
        assert_eq!(fx.fx_rate_to_dkk_micro, rate);
        assert_eq!(fx.net_amount_dkk_minor, 74_600);
        assert_eq!(fx.vat_amount_dkk_minor, 18_650);
        assert_eq!(fx.gross_amount_dkk_minor, 93_250);
    }

    #[test]
    fn dkk_invoice_omits_fx_totals() {
        let mut inv = eur_invoice(10_000, 2_500, 12_500, 7_460_000);
        inv.lines[0].currency = Currency::new("DKK").unwrap();
        inv.fx_rate_to_dkk_micro = None;
        assert!(fx_totals_for_invoice(&inv).unwrap().is_none());
    }

    #[test]
    fn non_dkk_requires_fx_rate() {
        let mut inv = eur_invoice(10_000, 2_500, 12_500, 7_460_000);
        inv.fx_rate_to_dkk_micro = None;
        assert!(matches!(
            fx_totals_for_invoice(&inv),
            Err(InvoiceError::MissingFxRate(_))
        ));
    }
}
