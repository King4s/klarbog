//! Stripe balance / payout CSV profile (comma-separated, RFC-ish quotes, ADR-009).

use crate::csv::{get_field, parse_row, validate_currencies, BankCsvError, BankRow};
use crate::revolut::split_csv_row;
use klarbog_types::Currency;

pub(crate) fn parse_stripe_csv(
    input: &str,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankCsvError> {
    let mut lines = input.lines().map(str::trim).filter(|l| !l.is_empty());
    let header = lines.next().ok_or(BankCsvError::Empty)?;
    let cols: Vec<String> = split_csv_row(header);
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let idx = stripe_column_map(&col_refs)?;
    let has_currency = idx.contains_key("currency");

    let mut out = Vec::new();
    let mut currencies: Vec<String> = Vec::new();
    for (i, line) in lines.enumerate() {
        let row_no = i + 2;
        let fields: Vec<String> = split_csv_row(line);
        let field_refs: Vec<&str> = fields.iter().map(String::as_str).collect();
        if has_currency {
            if let Some(raw) = get_field(&idx, &field_refs, "currency", row_no)? {
                let code = raw.trim().to_uppercase();
                Currency::new(&code).map_err(|_| BankCsvError::Row {
                    row: row_no,
                    detail: format!("invalid currency: {raw}"),
                })?;
                currencies.push(code);
            }
        }
        out.push(parse_row(&idx, &field_refs, row_no)?);
    }

    if has_currency {
        validate_currencies(&currencies, required_currency)?;
    }
    Ok(out)
}

fn stripe_column_map(
    header: &[&str],
) -> Result<std::collections::HashMap<&'static str, usize>, BankCsvError> {
    let mut map = std::collections::HashMap::new();
    for (i, cell) in header.iter().enumerate() {
        if let Some((key, pri)) = stripe_header_key(cell) {
            map.entry(key)
                .and_modify(|(ix, p)| {
                    if pri < *p {
                        *ix = i;
                        *p = pri;
                    }
                })
                .or_insert((i, pri));
        }
    }
    let flat: std::collections::HashMap<&'static str, usize> =
        map.into_iter().map(|(k, (ix, _))| (k, ix)).collect();
    for need in ["date", "text", "amount"] {
        if !flat.contains_key(need) {
            return Err(BankCsvError::Row {
                row: 1,
                detail: format!("header missing {need}"),
            });
        }
    }
    Ok(flat)
}

fn stripe_header_key(cell: &str) -> Option<(&'static str, u8)> {
    match cell.trim().to_ascii_lowercase().as_str() {
        "created" => Some(("date", 0)),
        "available on" => Some(("date", 1)),
        "date" => Some(("date", 2)),
        "description" => Some(("text", 0)),
        "type" => Some(("text", 1)),
        "reporting category" => Some(("text", 2)),
        "net" => Some(("amount", 0)),
        "amount" => Some(("amount", 1)),
        "currency" => Some(("currency", 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/stripe_balance.csv");

    #[test]
    fn stripe_fixture_prefers_net_over_amount() {
        let rows = parse_stripe_csv(FIXTURE, Some(&Currency::new("DKK").unwrap())).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].amount_minor.minor(), -100000);
        assert_eq!(rows[0].text, "Stripe payout");
        assert_eq!(rows[1].amount_minor.minor(), 24275);
        assert_eq!(rows[2].amount_minor.minor(), -5000);
    }

    #[test]
    fn stripe_rejects_mixed_currency() {
        let csv = "Created,Description,Net,Currency\n\
2026-01-01,A,-10.00,DKK\n2026-01-02,B,5.00,EUR\n";
        let err = parse_stripe_csv(csv, None).unwrap_err();
        assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
    }

    #[test]
    fn stripe_amount_only_when_no_net_column() {
        let csv = "Date,Description,Amount,Currency\n2026-01-01,Charge,99.50,DKK\n";
        let rows = parse_stripe_csv(csv, None).unwrap();
        assert_eq!(rows[0].amount_minor.minor(), 9950);
    }
}
