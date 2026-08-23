//! Fiscal-year-end + 5 years retention deadline (DK-BOOKKEEPING-RETENTION-001).
//! Calendar fiscal year (Jan–Dec) until company settings expose a start month.

use chrono::{Datelike, NaiveDate};
use thiserror::Error;

pub const RETENTION_RULE_ID: &str = "DK-BOOKKEEPING-RETENTION-001";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetentionDeadlineError {
    #[error("invalid ISO date: {0}")]
    InvalidIsoDate(String),
}

pub fn parse_iso_date(text: &str) -> Result<NaiveDate, RetentionDeadlineError> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|_| RetentionDeadlineError::InvalidIsoDate(text.to_string()))
}

/// End of calendar fiscal year for `basis`, plus five years (original default).
pub fn retain_until_for_date(basis: NaiveDate) -> NaiveDate {
    let year_end = NaiveDate::from_ymd_opt(basis.year(), 12, 31).expect("Dec 31 exists");
    NaiveDate::from_ymd_opt(year_end.year() + 5, 12, 31).expect("Dec 31 exists")
}

pub fn retain_until_iso(basis: NaiveDate) -> String {
    retain_until_for_date(basis).format("%Y-%m-%d").to_string()
}

pub fn retain_until_for_iso_date(basis: &str) -> Result<String, RetentionDeadlineError> {
    Ok(retain_until_iso(parse_iso_date(basis)?))
}

pub fn effective_retain_until(
    stored: Option<&str>,
    basis: Option<&str>,
) -> Result<Option<NaiveDate>, RetentionDeadlineError> {
    if let Some(value) = stored {
        if !value.is_empty() {
            return Ok(Some(parse_iso_date(value)?));
        }
    }
    if let Some(basis) = basis {
        if !basis.is_empty() {
            return Ok(Some(retain_until_for_date(parse_iso_date(basis)?)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mid_year_basis_ends_five_years_after_calendar_year_end() {
        let basis = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        assert_eq!(
            retain_until_for_date(basis),
            NaiveDate::from_ymd_opt(2031, 12, 31).unwrap()
        );
    }

    #[test]
    fn year_boundary_uses_basis_calendar_year() {
        let dec = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let jan = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        assert_eq!(
            retain_until_for_date(dec),
            NaiveDate::from_ymd_opt(2031, 12, 31).unwrap()
        );
        assert_eq!(
            retain_until_for_date(jan),
            NaiveDate::from_ymd_opt(2032, 12, 31).unwrap()
        );
    }

    #[test]
    fn effective_prefers_stored_deadline() {
        let effective = effective_retain_until(Some("2030-06-01"), Some("2026-01-01")).unwrap();
        assert_eq!(
            effective,
            Some(NaiveDate::from_ymd_opt(2030, 6, 1).unwrap())
        );
    }
}
