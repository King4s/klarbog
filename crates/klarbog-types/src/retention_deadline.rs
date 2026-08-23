//! Fiscal-year-end + 5 years retention deadline (DK-BOOKKEEPING-RETENTION-001).

use crate::fiscal_year::{fiscal_year_end, FiscalYearSettings};
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

/// End of company fiscal year for `basis`, plus five years (original default).
pub fn retain_until_for_date(basis: NaiveDate, settings: FiscalYearSettings) -> NaiveDate {
    let end = fiscal_year_end(basis, settings);
    NaiveDate::from_ymd_opt(end.year() + 5, end.month(), end.day()).expect("valid retain_until")
}

pub fn retain_until_iso(basis: NaiveDate, settings: FiscalYearSettings) -> String {
    retain_until_for_date(basis, settings)
        .format("%Y-%m-%d")
        .to_string()
}

pub fn retain_until_for_iso_date(
    basis: &str,
    settings: FiscalYearSettings,
) -> Result<String, RetentionDeadlineError> {
    Ok(retain_until_iso(parse_iso_date(basis)?, settings))
}

pub fn effective_retain_until(
    stored: Option<&str>,
    basis: Option<&str>,
    settings: FiscalYearSettings,
) -> Result<Option<NaiveDate>, RetentionDeadlineError> {
    if let Some(value) = stored {
        if !value.is_empty() {
            return Ok(Some(parse_iso_date(value)?));
        }
    }
    if let Some(basis) = basis {
        if !basis.is_empty() {
            return Ok(Some(retain_until_for_date(
                parse_iso_date(basis)?,
                settings,
            )));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fiscal_year::FiscalYearLabelStrategy;

    #[test]
    fn mid_year_basis_ends_five_years_after_calendar_year_end() {
        let basis = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        assert_eq!(
            retain_until_for_date(basis, FiscalYearSettings::default()),
            NaiveDate::from_ymd_opt(2031, 12, 31).unwrap()
        );
    }

    #[test]
    fn july_fiscal_year_end_is_june_thirty() {
        let settings = FiscalYearSettings {
            start_month: 7,
            label_strategy: FiscalYearLabelStrategy::EndYear,
        };
        let basis = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        assert_eq!(
            retain_until_for_date(basis, settings),
            NaiveDate::from_ymd_opt(2031, 6, 30).unwrap()
        );
    }

    #[test]
    fn effective_prefers_stored_deadline() {
        let effective = effective_retain_until(
            Some("2030-06-01"),
            Some("2026-01-01"),
            FiscalYearSettings::default(),
        )
        .unwrap();
        assert_eq!(
            effective,
            Some(NaiveDate::from_ymd_opt(2030, 6, 1).unwrap())
        );
    }
}
