//! Forfaldsdato og overdue-klassifikation — porteret fra originalens
//! `invoice-payments.ts` (DK-INVOICE-DUE-DATE-001).

use crate::InvoiceError;
use chrono::NaiveDate;

/// Lovlig standardfrist når ingen eksplicit forfaldsdato er sat (renteloven § 3).
pub const STATUTORY_PAYMENT_TERM_DAYS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceDueAssessment {
    pub due_date: Option<String>,
    pub effective_due_date: Option<String>,
    pub overdue_days: u32,
    pub is_overdue: bool,
}

pub fn parse_iso_date(raw: &str) -> Result<NaiveDate, InvoiceError> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| InvoiceError::InvalidDueDate(raw.trim().to_string()))
}

pub fn format_iso_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn add_days(date: NaiveDate, days: i64) -> NaiveDate {
    date + chrono::Duration::days(days)
}

/// Kalenderdage fra `from` til `to` (positiv når `to` er efter `from`).
pub fn diff_days(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days()
}

/// Eksplicit forfaldsdato eller udstedelsesdato + 30 dage (originalens
/// `getInvoiceStatus` effectiveDueDate).
pub fn effective_due_date(
    issue_date: Option<&str>,
    due_date: Option<&str>,
) -> Result<Option<String>, InvoiceError> {
    if let Some(explicit) = due_date.map(str::trim).filter(|s| !s.is_empty()) {
        let due = parse_iso_date(explicit)?;
        if let Some(issue) = issue_date.map(str::trim).filter(|s| !s.is_empty()) {
            let issue = parse_iso_date(issue)?;
            if due < issue {
                return Err(InvoiceError::DueBeforeIssue {
                    due_date: explicit.to_string(),
                    issue_date: issue.to_string(),
                });
            }
        }
        return Ok(Some(format_iso_date(due)));
    }
    let Some(issue) = issue_date.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let issue = parse_iso_date(issue)?;
    Ok(Some(format_iso_date(add_days(
        issue,
        STATUTORY_PAYMENT_TERM_DAYS,
    ))))
}

/// Overdue kun når der er positivt inddriveligt hovedstol (originalens
/// `openBalance > 0` gate).
pub fn assess_overdue(
    issue_date: Option<&str>,
    due_date: Option<&str>,
    collectible_open_minor: i64,
    as_of: NaiveDate,
) -> Result<InvoiceDueAssessment, InvoiceError> {
    let due = due_date
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let effective = effective_due_date(issue_date, due_date)?;
    let (overdue_days, is_overdue) = if collectible_open_minor <= 0 {
        (0, false)
    } else if let Some(ref eff) = effective {
        let due_naive = parse_iso_date(eff)?;
        let days = diff_days(due_naive, as_of).max(0);
        let overdue = days > 0;
        (days as u32, overdue)
    } else {
        (0, false)
    };
    Ok(InvoiceDueAssessment {
        due_date: due,
        effective_due_date: effective,
        overdue_days,
        is_overdue,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_due_date_is_used() {
        assert_eq!(
            effective_due_date(Some("2026-05-16"), Some("2026-06-15")).unwrap(),
            Some("2026-06-15".into())
        );
    }

    #[test]
    fn missing_due_falls_back_to_issue_plus_30() {
        assert_eq!(
            effective_due_date(Some("2026-05-16"), None).unwrap(),
            Some("2026-06-15".into())
        );
    }

    #[test]
    fn overdue_requires_positive_collectible_balance() {
        let as_of = parse_iso_date("2026-06-20").unwrap();
        let paid = assess_overdue(Some("2026-05-16"), Some("2026-06-15"), 0, as_of).unwrap();
        assert!(!paid.is_overdue);
        assert_eq!(paid.overdue_days, 0);

        let open = assess_overdue(Some("2026-05-16"), Some("2026-06-15"), 250, as_of).unwrap();
        assert!(open.is_overdue);
        assert_eq!(open.overdue_days, 5);
        assert_eq!(open.effective_due_date.as_deref(), Some("2026-06-15"));
    }

    #[test]
    fn due_before_issue_is_rejected() {
        let err = effective_due_date(Some("2026-06-01"), Some("2026-05-01")).unwrap_err();
        assert!(matches!(err, InvoiceError::DueBeforeIssue { .. }));
    }
}
