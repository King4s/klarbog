//! Statutory late interest (morarente) — porteret fra originalens
//! `invoice-interest.ts` (DK-INVOICE-LATE-INTEREST-001).

#[path = "late_interest_accrual.rs"]
mod late_interest_accrual;
#[path = "late_interest_post.rs"]
mod late_interest_post;

use crate::due_date::{assess_overdue, diff_days, format_iso_date, parse_iso_date};
use crate::{Invoice, InvoiceError, InvoiceId};
use chrono::NaiveDate;
use late_interest_accrual::{accrue_windows, claim_rate_windows, principal_open_minor, RateWindow};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub use late_interest_post::{
    interest_post_journal_suggestion, mark_interest_claim_posted, oldest_unposted_interest_claim,
    resolve_unposted_interest_claim,
};

pub const RULE_ID: &str = "DK-INVOICE-LATE-INTEREST-001";
pub const REGISTER_RULE_ID: &str = "DK-INVOICE-LATE-INTEREST-REGISTER-001";
pub const BOOKKEEPING_RULE_ID: &str = "DK-INVOICE-LATE-INTEREST-BOOKKEEPING-001";

/// Statutory surcharge (renteloven § 5, stk. 1): morarente = reference + 8 pct.
pub const STATUTORY_SURCHARGE_BPS: i64 = 800;

/// Manual reference rate sanity bound (percentage points × 100).
pub const REFERENCE_RATE_SANITY_TOLERANCE_BPS: i64 = 25;

/// Reference rate in hundredths of a percent (2.2 % → 220).
pub(crate) type RateBps = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceRateSource {
    StatutoryTable,
    ManualOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceInterestClaim {
    pub claim_date: String,
    pub reference_rate_bps: RateBps,
    pub annual_interest_rate_bps: RateBps,
    pub reference_rate_source: ReferenceRateSource,
    pub claimable_days: u32,
    pub principal_open_minor: i64,
    pub amount_minor: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_journal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterestSegment {
    pub principal_minor: i64,
    pub annual_rate_bps: RateBps,
    pub days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LateInterestCalculation {
    pub as_of_date: String,
    pub effective_due_date: Option<String>,
    pub overdue_days: u32,
    pub interest_from_date: Option<String>,
    pub claimable_days: u32,
    pub principal_open_minor: i64,
    pub reference_rate_bps: RateBps,
    pub annual_interest_rate_bps: RateBps,
    pub accrued_interest_minor: i64,
    pub prior_claimed_interest_minor: i64,
    pub total_interest_to_date_minor: i64,
    pub over_claimed_interest_minor: i64,
    pub reference_rate_source: ReferenceRateSource,
    pub warnings: Vec<String>,
}

const REFERENCE_RATE_TABLE: &[(&str, RateBps)] = &[
    ("2023-01-01", 190),
    ("2023-07-01", 325),
    ("2024-01-01", 375),
    ("2024-07-01", 350),
    ("2025-01-01", 275),
    ("2025-07-01", 175),
    ("2026-01-01", 175),
];

pub fn lookup_statutory_reference_rate(as_of: NaiveDate) -> Option<RateBps> {
    let as_of_s = format_iso_date(as_of);
    let mut matched = None;
    for (from, rate) in REFERENCE_RATE_TABLE {
        if *from <= as_of_s.as_str() {
            matched = Some(*rate);
        } else {
            break;
        }
    }
    matched
}

fn rate_from_reference(reference_bps: RateBps) -> RateBps {
    reference_bps + STATUTORY_SURCHARGE_BPS
}

fn round_div(numerator: i128, denominator: i128) -> Result<i64, InvoiceError> {
    if denominator == 0 {
        return Err(InvoiceError::Overflow);
    }
    let same_sign = (numerator >= 0 && denominator > 0) || (numerator <= 0 && denominator < 0);
    let abs_n = numerator.unsigned_abs();
    let abs_d = denominator.unsigned_abs();
    let quotient = abs_n / abs_d;
    let remainder = abs_n % abs_d;
    let rounded = if remainder * 2 >= abs_d {
        quotient + 1
    } else {
        quotient
    };
    let signed = if same_sign {
        rounded as i128
    } else {
        -(rounded as i128)
    };
    i64::try_from(signed).map_err(|_| InvoiceError::Overflow)
}

/// Cumulative statutory interest across segments, rounded to øre exactly once.
pub fn cumulative_interest_minor(segments: &[InterestSegment]) -> Result<i64, InvoiceError> {
    let mut numerator: i128 = 0;
    for s in segments {
        if s.days <= 0 || s.principal_minor <= 0 {
            continue;
        }
        numerator = numerator
            .checked_add(
                i128::from(s.principal_minor) * i128::from(s.annual_rate_bps) * i128::from(s.days),
            )
            .ok_or(InvoiceError::Overflow)?;
    }
    round_div(numerator, 10_000 * 365)
}

pub fn calculate_late_interest(
    invoice: &Invoice,
    as_of: NaiveDate,
    manual_reference_bps: Option<RateBps>,
) -> Result<LateInterestCalculation, InvoiceError> {
    if manual_reference_bps.is_some_and(|r| r < 0) {
        return Err(InvoiceError::InvalidReferenceRate);
    }

    let mut warnings = Vec::new();
    let table_rate = lookup_statutory_reference_rate(as_of);
    let (reference_rate_bps, reference_rate_source) = match manual_reference_bps {
        None => {
            let Some(rate) = table_rate else {
                return Err(InvoiceError::NoStatutoryReferenceRate(format_iso_date(
                    as_of,
                )));
            };
            (rate, ReferenceRateSource::StatutoryTable)
        }
        Some(manual) => {
            if let Some(table) = table_rate {
                if (manual - table).abs() > REFERENCE_RATE_SANITY_TOLERANCE_BPS {
                    warnings.push(format!(
                        "manual reference rate {manual} bps deviates from statutory {table} bps for half-year containing {}",
                        format_iso_date(as_of)
                    ));
                }
            }
            (manual, ReferenceRateSource::ManualOverride)
        }
    };

    let annual_interest_rate_bps = rate_from_reference(reference_rate_bps);
    let collectible = invoice.collectible_open_minor()?;
    let due = assess_overdue(
        invoice.issue_date.as_deref(),
        invoice.due_date.as_deref(),
        collectible,
        as_of,
    )?;
    let effective_due = due
        .effective_due_date
        .as_deref()
        .map(parse_iso_date)
        .transpose()?;
    let principal_open_minor = principal_open_minor(invoice, as_of)?;

    let prior_claims = &invoice.interest_claims;
    let last_claim_date = prior_claims
        .last()
        .map(|c| parse_iso_date(&c.claim_date))
        .transpose()?;

    let anchor = match (effective_due, last_claim_date) {
        (Some(eff), Some(last)) if last > eff => Some(last),
        (Some(eff), _) => Some(eff),
        _ => None,
    };
    let interest_from_date = anchor.map(|d| if d > as_of { as_of } else { d });
    let claimable_days = if principal_open_minor > 0 {
        interest_from_date
            .map(|from| diff_days(from, as_of).max(0) as u32)
            .unwrap_or(0)
    } else {
        0
    };

    let mut windows: Vec<RateWindow> = Vec::new();
    let mut prior_claimed_interest_minor: i64 = 0;
    for claim in prior_claims {
        let claim_date = parse_iso_date(&claim.claim_date)?;
        let end = if claim_date < as_of {
            claim_date
        } else {
            as_of
        };
        windows.push(RateWindow {
            end,
            annual_rate_bps: claim.annual_interest_rate_bps,
        });
        if claim_date <= as_of {
            prior_claimed_interest_minor = prior_claimed_interest_minor
                .checked_add(claim.amount_minor)
                .ok_or(InvoiceError::Overflow)?;
        }
    }

    if claimable_days > 0 {
        if let Some(from) = interest_from_date {
            if reference_rate_source == ReferenceRateSource::StatutoryTable {
                windows.extend(claim_rate_windows(
                    from,
                    as_of,
                    reference_rate_source,
                    annual_interest_rate_bps,
                    reference_rate_bps,
                ));
            } else {
                windows.push(RateWindow {
                    end: as_of,
                    annual_rate_bps: annual_interest_rate_bps,
                });
            }
        }
    }

    let total_interest_to_date_minor = accrue_windows(invoice, effective_due, &windows)?;
    let accrued_interest_minor =
        if claimable_days > 0 && total_interest_to_date_minor > prior_claimed_interest_minor {
            total_interest_to_date_minor - prior_claimed_interest_minor
        } else {
            0
        };
    let over_claimed_interest_minor = if prior_claimed_interest_minor > total_interest_to_date_minor
    {
        prior_claimed_interest_minor - total_interest_to_date_minor
    } else {
        0
    };

    Ok(LateInterestCalculation {
        as_of_date: format_iso_date(as_of),
        effective_due_date: due.effective_due_date,
        overdue_days: due.overdue_days,
        interest_from_date: interest_from_date.map(format_iso_date),
        claimable_days,
        principal_open_minor,
        reference_rate_bps,
        annual_interest_rate_bps,
        accrued_interest_minor,
        prior_claimed_interest_minor,
        total_interest_to_date_minor,
        over_claimed_interest_minor,
        reference_rate_source,
        warnings,
    })
}

pub fn register_late_interest(
    company: &Path,
    id: &InvoiceId,
    as_of: NaiveDate,
    reference_rate_bps: Option<RateBps>,
    note: Option<String>,
) -> Result<(Invoice, LateInterestCalculation), InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    if !invoice.status.allows_late_interest() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: crate::InvoiceStatus::Sent,
        });
    }

    let calc = calculate_late_interest(invoice, as_of, reference_rate_bps)?;
    for claim in &invoice.interest_claims {
        if claim.claim_date == calc.as_of_date
            && claim.reference_rate_bps == calc.reference_rate_bps
        {
            return Err(InvoiceError::DuplicateInterestClaim {
                claim_date: calc.as_of_date.clone(),
                reference_rate_bps: calc.reference_rate_bps,
            });
        }
    }
    if calc.accrued_interest_minor <= 0 {
        return Err(InvoiceError::NoInterestToRegister);
    }

    let claim = InvoiceInterestClaim {
        claim_date: calc.as_of_date.clone(),
        reference_rate_bps: calc.reference_rate_bps,
        annual_interest_rate_bps: calc.annual_interest_rate_bps,
        reference_rate_source: calc.reference_rate_source,
        claimable_days: calc.claimable_days,
        principal_open_minor: calc.principal_open_minor,
        amount_minor: calc.accrued_interest_minor,
        note,
        posted_journal_id: None,
    };
    invoice.interest_claims.push(claim.clone());
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    crate::claim_ledger::dual_write_interest(company, id, &claim)?;
    Ok((updated, calc))
}

pub fn total_interest_claims_minor(invoice: &Invoice) -> Result<i64, InvoiceError> {
    invoice.interest_claims.iter().try_fold(0i64, |acc, c| {
        acc.checked_add(c.amount_minor)
            .ok_or(InvoiceError::Overflow)
    })
}

pub fn claim_open_balance_minor(invoice: &Invoice) -> Result<i64, InvoiceError> {
    let base = invoice
        .collectible_open_minor()?
        .checked_add(total_interest_claims_minor(invoice)?)
        .ok_or(InvoiceError::Overflow)?;
    let with_reminders = base
        .checked_add(crate::reminders::total_reminder_fees_minor(invoice)?)
        .ok_or(InvoiceError::Overflow)?;
    with_reminders
        .checked_add(crate::late_compensation::total_compensation_minor(invoice)?)
        .ok_or(InvoiceError::Overflow)
}
