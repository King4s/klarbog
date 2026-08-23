//! Fiscal year boundaries and sequence labels (original `fiscal-year.ts`).

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FiscalYearLabelStrategy {
    #[default]
    #[serde(rename = "end-year")]
    EndYear,
    #[serde(rename = "start-year")]
    StartYear,
    Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiscalYearSettings {
    #[serde(rename = "fiscalYearStartMonth", default = "default_start_month")]
    pub start_month: u32,
    #[serde(rename = "fiscalYearLabelStrategy", default = "default_label_strategy")]
    pub label_strategy: FiscalYearLabelStrategy,
}

impl Default for FiscalYearSettings {
    fn default() -> Self {
        Self {
            start_month: default_start_month(),
            label_strategy: default_label_strategy(),
        }
    }
}

const fn default_start_month() -> u32 {
    1
}

const fn default_label_strategy() -> FiscalYearLabelStrategy {
    FiscalYearLabelStrategy::EndYear
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiscalYear {
    pub start_year: i32,
    pub end_year: i32,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub display_label: String,
    pub identifier_label: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FiscalYearError {
    #[error("invalid ISO date: {0}")]
    InvalidIsoDate(String),
    #[error("fiscalYearStartMonth must be 1..=12")]
    InvalidStartMonth,
}

pub fn normalize_start_month(month: u32) -> u32 {
    if (1..=12).contains(&month) {
        month
    } else {
        1
    }
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1)
        .and_then(|d| d.pred_opt())
        .expect("valid month boundary")
}

pub fn fiscal_year_for_date(
    basis: NaiveDate,
    settings: FiscalYearSettings,
) -> Result<FiscalYear, FiscalYearError> {
    let start_month = normalize_start_month(settings.start_month);
    let year = basis.year();
    let month = basis.month();
    let start_year = if start_month == 1 || month >= start_month {
        year
    } else {
        year - 1
    };
    let end_year = if start_month == 1 {
        start_year
    } else {
        start_year + 1
    };
    let start = NaiveDate::from_ymd_opt(start_year, start_month, 1).expect("valid start");
    let end = if start_month == 1 {
        NaiveDate::from_ymd_opt(end_year, 12, 31).expect("valid end")
    } else {
        last_day_of_month(end_year, start_month - 1)
    };
    let display_label = match settings.label_strategy {
        FiscalYearLabelStrategy::StartYear => start_year.to_string(),
        FiscalYearLabelStrategy::Span if start_month != 1 => {
            format!("{start_year}/{:02}", end_year % 100)
        }
        FiscalYearLabelStrategy::EndYear | FiscalYearLabelStrategy::Span => end_year.to_string(),
    };
    let identifier_label = display_label.replace('/', "-");
    Ok(FiscalYear {
        start_year,
        end_year,
        start,
        end,
        display_label,
        identifier_label,
    })
}

pub fn fiscal_year_for_iso_date(
    date_text: &str,
    settings: FiscalYearSettings,
) -> Result<FiscalYear, FiscalYearError> {
    let basis = NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|_| FiscalYearError::InvalidIsoDate(date_text.to_string()))?;
    fiscal_year_for_date(basis, settings)
}

pub fn fiscal_year_identifier_label(basis: NaiveDate, settings: FiscalYearSettings) -> String {
    fiscal_year_for_date(basis, settings)
        .map(|fy| fy.identifier_label)
        .unwrap_or_else(|_| basis.year().to_string())
}

pub fn fiscal_year_end(basis: NaiveDate, settings: FiscalYearSettings) -> NaiveDate {
    fiscal_year_for_date(basis, settings)
        .map(|fy| fy.end)
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(basis.year(), 12, 31).expect("Dec 31 exists"))
}

/// Read fiscal settings from `policy.json` (defaults when missing/invalid).
pub fn load_fiscal_settings(company: &Path) -> FiscalYearSettings {
    let path = company.join("policy.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return FiscalYearSettings::default();
    };
    #[derive(Deserialize)]
    struct PolicyFiscal {
        #[serde(rename = "fiscalYearStartMonth", default = "default_start_month")]
        fiscal_year_start_month: u32,
        #[serde(rename = "fiscalYearLabelStrategy", default = "default_label_strategy")]
        fiscal_year_label_strategy: FiscalYearLabelStrategy,
    }
    serde_json::from_str::<PolicyFiscal>(&raw)
        .map(|p| FiscalYearSettings {
            start_month: normalize_start_month(p.fiscal_year_start_month),
            label_strategy: p.fiscal_year_label_strategy,
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn calendar_year_matches_original_defaults() {
        let settings = FiscalYearSettings::default();
        let fy = fiscal_year_for_date(d(2026, 5, 20), settings).unwrap();
        assert_eq!(fy.end, d(2026, 12, 31));
        assert_eq!(fy.identifier_label, "2026");
    }

    #[test]
    fn july_start_end_year_label() {
        let settings = FiscalYearSettings {
            start_month: 7,
            label_strategy: FiscalYearLabelStrategy::EndYear,
        };
        let may = fiscal_year_for_date(d(2026, 5, 20), settings).unwrap();
        assert_eq!(may.end, d(2026, 6, 30));
        assert_eq!(may.identifier_label, "2026");
        let aug = fiscal_year_for_date(d(2026, 8, 15), settings).unwrap();
        assert_eq!(aug.end, d(2027, 6, 30));
        assert_eq!(aug.identifier_label, "2027");
    }

    #[test]
    fn span_label_for_non_calendar_year() {
        let settings = FiscalYearSettings {
            start_month: 7,
            label_strategy: FiscalYearLabelStrategy::Span,
        };
        let fy = fiscal_year_for_date(d(2026, 8, 1), settings).unwrap();
        assert_eq!(fy.display_label, "2026/27");
        assert_eq!(fy.identifier_label, "2026-27");
    }
}
