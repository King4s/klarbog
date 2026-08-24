//! Fortløbende fakturanumre `{scope}-{NNNN}` (DK-INVOICE-ISSUE-001).

use crate::{Invoice, InvoiceError};
use chrono::{DateTime, Utc};
use klarbog_types::{fiscal_year_identifier_label, load_fiscal_settings};
use std::path::Path;

use crate::sequences::{peek_value, reserve_value};

fn fiscal_scope(company: &Path, as_of: DateTime<Utc>) -> String {
    let settings = load_fiscal_settings(company);
    fiscal_year_identifier_label(as_of.date_naive(), settings)
}

fn format_invoice_no(scope: &str, value: u32) -> String {
    format!("{scope}-{value:04}")
}

fn parse_canonical_invoice_no(number: &str) -> Option<(&str, u32)> {
    let (scope, suffix) = number.trim().rsplit_once('-')?;
    if suffix.len() != 4 || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((scope, suffix.parse().ok()?))
}

fn issued_invoice_floor(invoices: &[Invoice], scope: &str) -> u32 {
    let mut max = 0u32;
    for inv in invoices {
        if let Some(no) = inv.invoice_no.as_deref() {
            if let Some((s, n)) = parse_canonical_invoice_no(no) {
                if s == scope {
                    max = max.max(n);
                }
            }
        }
    }
    max
}

pub fn validate_manual_invoice_number_scope(
    company: &Path,
    as_of: DateTime<Utc>,
    invoice_number: &str,
) -> Result<(), InvoiceError> {
    if let Some((year, _)) = parse_canonical_invoice_no(invoice_number.trim()) {
        let scope = fiscal_scope(company, as_of);
        if year != scope {
            return Err(InvoiceError::ManualInvoiceScopeMismatch {
                number: invoice_number.trim().to_string(),
                scope,
            });
        }
    }
    Ok(())
}

pub fn peek_invoice_number(company: &Path, as_of: DateTime<Utc>) -> Result<String, InvoiceError> {
    let scope = fiscal_scope(company, as_of);
    let invoices = crate::store::list_invoices(company)?;
    let floor = issued_invoice_floor(&invoices, &scope);
    let value = peek_value(company, "issued_invoice", &scope, floor)?;
    Ok(format_invoice_no(&scope, value))
}

pub fn resolve_invoice_number(
    company: &Path,
    as_of: DateTime<Utc>,
    manual: Option<&str>,
) -> Result<String, InvoiceError> {
    if let Some(raw) = manual.map(str::trim).filter(|s| !s.is_empty()) {
        validate_manual_invoice_number_scope(company, as_of, raw)?;
        Ok(raw.to_string())
    } else {
        peek_invoice_number(company, as_of)
    }
}

pub fn reserve_invoice_number(
    company: &Path,
    as_of: DateTime<Utc>,
    number: &str,
) -> Result<(), InvoiceError> {
    let scope = fiscal_scope(company, as_of);
    let Some((s, value)) = parse_canonical_invoice_no(number) else {
        return Ok(());
    };
    if s != scope {
        return Err(InvoiceError::BadInvoiceNumber(number.to_string()));
    }
    let invoices = crate::store::list_invoices(company)?;
    let floor = issued_invoice_floor(&invoices, &scope);
    reserve_value(company, "issued_invoice", &scope, value, floor)
}

/// Memo: `invoice:{id}:issued:{2026-0001} · {description}`.
pub fn invoice_no_from_memo(memo: &str) -> Option<String> {
    let tail = memo.split(":issued:").nth(1)?;
    let no = tail.split(" · ").next()?.trim();
    if no.contains('-') && !no.starts_with("CN-") {
        Some(no.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn dt(s: &str) -> DateTime<Utc> {
        format!("{s}T12:00:00Z").parse().unwrap()
    }

    #[test]
    fn peek_and_reserve_advances() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let as_of = dt("2026-05-20");
        assert_eq!(peek_invoice_number(&co, as_of).unwrap(), "2026-0001");
        reserve_invoice_number(&co, as_of, "2026-0001").unwrap();
        assert_eq!(peek_invoice_number(&co, as_of).unwrap(), "2026-0002");
    }

    #[test]
    fn memo_roundtrip() {
        assert_eq!(
            invoice_no_from_memo("invoice:inv_1:issued:2026-0007 · Smoke").as_deref(),
            Some("2026-0007")
        );
        assert_eq!(
            invoice_no_from_memo("invoice:inv_1:credit:CN-2026-0001 · x"),
            None
        );
    }
}
