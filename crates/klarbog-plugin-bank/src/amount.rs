//! Parse bank amount cells into integer minor units (never f64).

use klarbog_types::{MinorAmount, MoneyError};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankAmountError {
    #[error("empty amount")]
    Empty,
    #[error("invalid amount: {0}")]
    Invalid(String),
    #[error(transparent)]
    Money(#[from] MoneyError),
}

/// Parse a Danish-ish amount cell: integer øre, or decimal kr with comma/dot.
pub fn parse_amount_minor(raw: &str) -> Result<MinorAmount, BankAmountError> {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return Err(BankAmountError::Empty);
    }
    if let Some(rest) = s.strip_suffix(" DKK") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_suffix(" dkk") {
        s = rest.to_string();
    }
    s = s.trim().to_string();

    let mut sign: i128 = 1;
    if let Some(inner) = s.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        sign = -1;
        s = inner.trim().to_string();
    } else if s.ends_with('-') && !s.starts_with('-') && !s.starts_with('+') {
        sign = -1;
        s = s.trim_end_matches('-').trim().to_string();
    } else if let Some(rest) = s.strip_prefix('-') {
        sign = -1;
        s = rest.trim().to_string();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.trim().to_string();
    }

    s = s.replace(' ', "");
    if s.chars().all(|c| c.is_ascii_digit()) {
        let units: i64 = s
            .parse()
            .map_err(|_| BankAmountError::Invalid(raw.into()))?;
        return Ok(MinorAmount::from_minor(sign as i64 * units));
    }

    let compact = if s.contains(',') {
        if !is_danish_decimal(&s) {
            return Err(BankAmountError::Invalid(raw.into()));
        }
        s.replace('.', "")
    } else if is_thousands_dots_only(&s) {
        s.replace('.', "")
    } else {
        s.to_string()
    };

    let (whole, frac, frac_len) = split_decimal(&compact)?;
    let minor = decimal_to_minor(whole, frac, frac_len)?;
    i64::try_from(sign * minor)
        .map(MinorAmount::from_minor)
        .map_err(|_| BankAmountError::Money(MoneyError::Overflow))
}

fn is_danish_decimal(s: &str) -> bool {
    let Some(idx) = s.rfind(',') else {
        return false;
    };
    let (left, right) = s.split_at(idx);
    left.chars()
        .chain(right[1..].chars())
        .all(|c| c.is_ascii_digit() || c == '.')
}

fn is_thousands_dots_only(s: &str) -> bool {
    if !s.contains('.') || s.contains(',') {
        return false;
    }
    if !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }
    // One dot with 1–2 trailing digits → decimal (Revolut / ISO), not thousands.
    if let Some((_, frac)) = s.rsplit_once('.') {
        if frac.len() <= 2 {
            return false;
        }
    }
    true
}

fn split_decimal(s: &str) -> Result<(i128, i128, u32), BankAmountError> {
    if let Some((w, f)) = s.split_once(',') {
        let whole = parse_digits(w)?;
        let frac = parse_digits(f)?;
        let frac_len = f.len() as u32;
        return Ok((whole, frac, frac_len));
    }
    if let Some((w, f)) = s.split_once('.') {
        let whole = parse_digits(w)?;
        let frac = parse_digits(f)?;
        let frac_len = f.len() as u32;
        return Ok((whole, frac, frac_len));
    }
    Ok((parse_digits(s)?, 0, 0))
}

fn parse_digits(s: &str) -> Result<i128, BankAmountError> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        return Err(BankAmountError::Invalid(s.into()));
    }
    s.parse().map_err(|_| BankAmountError::Invalid(s.into()))
}

fn decimal_to_minor(whole: i128, frac: i128, frac_len: u32) -> Result<i128, BankAmountError> {
    if frac_len <= 2 {
        let scale = 10i128.pow(2 - frac_len);
        let minor = whole
            .checked_mul(100)
            .and_then(|w| w.checked_add(frac.checked_mul(scale)?))
            .ok_or(BankAmountError::Money(MoneyError::Overflow))?;
        return Ok(minor);
    }
    let numer = whole
        .checked_mul(10i128.pow(frac_len))
        .and_then(|w| w.checked_add(frac))
        .ok_or(BankAmountError::Money(MoneyError::Overflow))?;
    let denom = 10i128.pow(frac_len);
    MinorAmount::from_ratio_half_even(numer * 100, denom)
        .map(|m| m.minor() as i128)
        .map_err(BankAmountError::Money)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_decimal() {
        assert_eq!(parse_amount_minor("-125,50").unwrap().minor(), -12550);
    }

    #[test]
    fn dot_decimal_revolut_style() {
        assert_eq!(parse_amount_minor("-45.50").unwrap().minor(), -4550);
        assert_eq!(parse_amount_minor("120.00").unwrap().minor(), 12000);
    }

    #[test]
    fn thousands_dots_still_stripped() {
        assert_eq!(parse_amount_minor("1.234").unwrap().minor(), 123400);
    }

    #[test]
    fn half_even_extra_decimals() {
        assert_eq!(parse_amount_minor("-10,005").unwrap().minor(), -1000);
        assert_eq!(parse_amount_minor("10,015").unwrap().minor(), 1002);
    }
}
