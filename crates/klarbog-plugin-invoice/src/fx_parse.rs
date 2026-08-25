//! Parse FX rates as integer micro-units (no f64).

use crate::InvoiceError;

/// Parse a decimal FX rate string to micro-units (6 decimal places, half-even).
pub fn parse_fx_rate_to_dkk_micro(raw: &str) -> Result<i64, InvoiceError> {
    let s = raw.trim();
    if s.is_empty() || s == "." {
        return Err(InvoiceError::InvalidFxRate);
    }
    if s.starts_with('-') || s.starts_with('+') {
        return Err(InvoiceError::InvalidFxRate);
    }
    let (whole, frac) = match s.split_once('.') {
        None => (s, ""),
        Some((w, f)) => (w, f),
    };
    if whole.is_empty() && frac.is_empty() {
        return Err(InvoiceError::InvalidFxRate);
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err(InvoiceError::InvalidFxRate);
    }
    let whole_part: i128 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| InvoiceError::InvalidFxRate)?
    };
    let micro_frac = frac_to_micro(frac)?;
    let out = whole_part
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(i128::from(micro_frac)))
        .ok_or(InvoiceError::Overflow)?;
    i64::try_from(out).map_err(|_| InvoiceError::Overflow)
}

fn frac_to_micro(frac: &str) -> Result<i64, InvoiceError> {
    if frac.is_empty() {
        return Ok(0);
    }
    if frac.len() <= 6 {
        let padded = format!("{frac:0<6}");
        return padded.parse().map_err(|_| InvoiceError::InvalidFxRate);
    }
    let keep = &frac[..6];
    let next = frac.as_bytes()[6];
    let mut micro: i64 = keep.parse().map_err(|_| InvoiceError::InvalidFxRate)?;
    if next >= b'5' && (next > b'5' || micro.rem_euclid(2) != 0) {
        micro = micro.checked_add(1).ok_or(InvoiceError::Overflow)?;
    }
    Ok(micro)
}

/// Fail-closed FX rules: DKK omits rate; foreign currency requires positive micro rate.
pub fn resolve_fx_rate_for_currency(
    currency: &str,
    fx_rate_to_dkk_micro: Option<i64>,
) -> Result<Option<i64>, InvoiceError> {
    let cur = currency.trim().to_uppercase();
    match (cur.as_str(), fx_rate_to_dkk_micro) {
        ("DKK", None) => Ok(None),
        ("DKK", Some(_)) => Err(InvoiceError::UnexpectedFxRateForDkk),
        (_, None) => Err(InvoiceError::MissingFxRate(cur)),
        (_, Some(rate)) if rate <= 0 => Err(InvoiceError::InvalidFxRate),
        (_, Some(rate)) => Ok(Some(rate)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_rates() {
        assert_eq!(parse_fx_rate_to_dkk_micro("7.46").unwrap(), 7_460_000);
        assert_eq!(parse_fx_rate_to_dkk_micro("7.455556").unwrap(), 7_455_556);
        assert_eq!(parse_fx_rate_to_dkk_micro("1").unwrap(), 1_000_000);
    }

    #[test]
    fn rounds_half_even_on_seventh_digit() {
        assert_eq!(parse_fx_rate_to_dkk_micro("7.4555555").unwrap(), 7_455_556);
    }

    #[test]
    fn resolve_fx_rules() {
        assert!(resolve_fx_rate_for_currency("DKK", None).unwrap().is_none());
        assert!(matches!(
            resolve_fx_rate_for_currency("DKK", Some(7_460_000)),
            Err(InvoiceError::UnexpectedFxRateForDkk)
        ));
        assert!(matches!(
            resolve_fx_rate_for_currency("EUR", None),
            Err(InvoiceError::MissingFxRate(_))
        ));
        assert_eq!(
            resolve_fx_rate_for_currency("EUR", Some(7_460_000)).unwrap(),
            Some(7_460_000)
        );
    }
}
