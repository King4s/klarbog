//! Bank CSV parsers — GenericDk (semicolon) and Revolut (comma, RFC-ish quotes).

use crate::amount::{parse_amount_minor, BankAmountError};
use chrono::{DateTime, NaiveDate, Utc};
use klarbog_types::Currency;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BankProfile {
    GenericDk,
    Revolut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankRow {
    pub date: DateTime<Utc>,
    pub text: String,
    pub amount_minor: klarbog_types::MinorAmount,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankCsvError {
    #[error("empty csv")]
    Empty,
    #[error("row {row}: {detail}")]
    Row { row: usize, detail: String },
    #[error("mixed currencies in batch: {found}")]
    MixedCurrency { found: String },
    #[error("currency mismatch: expected {expected}, found {found}")]
    CurrencyMismatch { expected: String, found: String },
    #[error(transparent)]
    Amount(#[from] BankAmountError),
}

pub fn parse_bank_csv(input: &str) -> Result<Vec<BankRow>, BankCsvError> {
    parse_bank_csv_with_profile(BankProfile::GenericDk, input, None)
}

pub fn parse_revolut_csv(
    input: &str,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankCsvError> {
    parse_bank_csv_with_profile(BankProfile::Revolut, input, required_currency)
}

pub fn parse_bank_csv_with_profile(
    profile: BankProfile,
    input: &str,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankCsvError> {
    match profile {
        BankProfile::GenericDk => parse_generic_dk_csv(input),
        BankProfile::Revolut => crate::revolut::parse_revolut_csv(input, required_currency),
    }
}

fn parse_generic_dk_csv(input: &str) -> Result<Vec<BankRow>, BankCsvError> {
    let mut lines = input.lines().map(str::trim).filter(|l| !l.is_empty());
    let header = lines.next().ok_or(BankCsvError::Empty)?;
    let cols = split_semicolon_row(header);
    let idx = generic_column_map(&cols)?;

    let mut out = Vec::new();
    for (i, line) in lines.enumerate() {
        let row_no = i + 2;
        let fields = split_semicolon_row(line);
        out.push(parse_row(&idx, &fields, row_no)?);
    }
    Ok(out)
}

pub(crate) fn parse_row(
    idx: &std::collections::HashMap<&'static str, usize>,
    fields: &[&str],
    row_no: usize,
) -> Result<BankRow, BankCsvError> {
    let date_raw = pick_field(idx, fields, &["date", "date_alt"], row_no)?;
    let text_raw = pick_field(idx, fields, &["text", "text_alt", "text_alt2"], row_no)?;
    let amount_raw =
        get_field(idx, fields, "amount", row_no)?.ok_or_else(|| BankCsvError::Row {
            row: row_no,
            detail: "missing column amount".into(),
        })?;
    let date = parse_date(date_raw).map_err(|e| BankCsvError::Row {
        row: row_no,
        detail: e,
    })?;
    let amount_minor = parse_amount_minor(amount_raw).map_err(|e| BankCsvError::Row {
        row: row_no,
        detail: e.to_string(),
    })?;
    Ok(BankRow {
        date,
        text: text_raw.to_string(),
        amount_minor,
    })
}

fn pick_field<'a>(
    idx: &std::collections::HashMap<&'static str, usize>,
    fields: &[&'a str],
    keys: &[&'static str],
    row_no: usize,
) -> Result<&'a str, BankCsvError> {
    for key in keys {
        if let Some(raw) = get_field(idx, fields, key, row_no)? {
            if !raw.trim().is_empty() {
                return Ok(raw);
            }
        }
    }
    Err(BankCsvError::Row {
        row: row_no,
        detail: format!("missing column {}", keys[0]),
    })
}

pub(crate) fn get_field<'a>(
    idx: &std::collections::HashMap<&'static str, usize>,
    fields: &[&'a str],
    key: &str,
    row_no: usize,
) -> Result<Option<&'a str>, BankCsvError> {
    let Some(ix) = idx.get(key) else {
        return Ok(None);
    };
    fields
        .get(*ix)
        .copied()
        .ok_or_else(|| BankCsvError::Row {
            row: row_no,
            detail: format!("short row for {key}"),
        })
        .map(Some)
}

fn split_semicolon_row(line: &str) -> Vec<&str> {
    line.split(';').map(str::trim).collect()
}

fn generic_column_map(
    header: &[&str],
) -> Result<std::collections::HashMap<&'static str, usize>, BankCsvError> {
    let mut map = std::collections::HashMap::new();
    for (i, cell) in header.iter().enumerate() {
        if let Some(key) = generic_header_key(cell) {
            map.insert(key, i);
        }
    }
    for need in ["date", "text", "amount"] {
        if !map.contains_key(need) {
            return Err(BankCsvError::Row {
                row: 1,
                detail: format!("header missing {need}"),
            });
        }
    }
    Ok(map)
}

fn generic_header_key(cell: &str) -> Option<&'static str> {
    match cell.trim().to_ascii_lowercase().as_str() {
        "dato" | "date" | "transaktionsdato" => Some("date"),
        "tekst" | "text" | "beskrivelse" => Some("text"),
        "beløb" | "belob" | "amount" | "beloeb" => Some("amount"),
        _ => None,
    }
}

fn parse_date(raw: &str) -> Result<DateTime<Utc>, String> {
    let s = raw.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    let nd = if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        d
    } else if let Ok(d) = NaiveDate::parse_from_str(s, "%d-%m-%Y") {
        d
    } else if let Ok(d) = NaiveDate::parse_from_str(s, "%d.%m.%Y") {
        d
    } else {
        return Err(format!("invalid date: {s}"));
    };
    nd.and_hms_opt(0, 0, 0)
        .ok_or_else(|| format!("invalid date: {s}"))
        .map(|t| t.and_utc())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_profile_snake_case() {
        let v = serde_json::to_value(BankProfile::Revolut).unwrap();
        assert_eq!(v, "revolut");
    }
}
