//! Revolut bank CSV profile (comma-separated, RFC-ish quotes, ADR-008).

use crate::csv::{get_field, parse_row, BankCsvError, BankRow};
use klarbog_types::Currency;

pub(crate) fn parse_revolut_csv(
    input: &str,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankCsvError> {
    let mut lines = input.lines().map(str::trim).filter(|l| !l.is_empty());
    let header = lines.next().ok_or(BankCsvError::Empty)?;
    let cols: Vec<String> = split_csv_row(header);
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let idx = revolut_column_map(&col_refs)?;
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

fn validate_currencies(
    currencies: &[String],
    required: Option<&Currency>,
) -> Result<(), BankCsvError> {
    if currencies.is_empty() {
        return Ok(());
    }
    let first = &currencies[0];
    if currencies.iter().any(|c| c != first) {
        let found: Vec<&str> = currencies.iter().map(String::as_str).collect();
        let mut uniq = found.clone();
        uniq.sort_unstable();
        uniq.dedup();
        return Err(BankCsvError::MixedCurrency {
            found: uniq.join(", "),
        });
    }
    if let Some(exp) = required {
        if first != exp.as_str() {
            return Err(BankCsvError::CurrencyMismatch {
                expected: exp.as_str().into(),
                found: first.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn split_csv_row(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if !in_quotes => in_quotes = true,
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            }
            ',' if !in_quotes => {
                out.push(field.trim().to_string());
                field.clear();
            }
            _ => field.push(c),
        }
    }
    out.push(field.trim().to_string());
    out
}

fn revolut_column_map(
    header: &[&str],
) -> Result<std::collections::HashMap<&'static str, usize>, BankCsvError> {
    let mut map = std::collections::HashMap::new();
    for (i, cell) in header.iter().enumerate() {
        if let Some((key, pri)) = revolut_header_key(cell) {
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

fn revolut_header_key(cell: &str) -> Option<(&'static str, u8)> {
    match cell.trim().to_ascii_lowercase().as_str() {
        "completed date" => Some(("date", 0)),
        "date" => Some(("date", 1)),
        "started date" => Some(("date_alt", 2)),
        "description" => Some(("text", 0)),
        "reference" => Some(("text_alt", 1)),
        "payment reference" => Some(("text_alt2", 2)),
        "amount" => Some(("amount", 0)),
        "currency" => Some(("currency", 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_csv_quoted_comma() {
        let fields = split_csv_row(r#""Transfer, internal",-200.00"#);
        assert_eq!(fields[0], "Transfer, internal");
    }
}
