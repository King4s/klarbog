//! Date-aware morarente accrual primitives (internal to late_interest).

use crate::due_date::diff_days;
use crate::{Invoice, InvoiceError};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};

use super::{
    cumulative_interest_minor, lookup_statutory_reference_rate, InterestSegment, RateBps,
    ReferenceRateSource, STATUTORY_SURCHARGE_BPS,
};

pub(super) struct RateWindow {
    pub end: NaiveDate,
    pub annual_rate_bps: RateBps,
}

struct BalanceEvent {
    date: NaiveDate,
    delta_minor: i64,
}

pub(super) fn rate_from_reference(reference_bps: RateBps) -> RateBps {
    reference_bps + STATUTORY_SURCHARGE_BPS
}

fn statutory_rate_boundaries_between(from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    for year in from.year()..=to.year() {
        for md in [(1, 1), (7, 1)] {
            if let Some(d) = NaiveDate::from_ymd_opt(year, md.0, md.1) {
                if d > from && d < to {
                    out.push(d);
                }
            }
        }
    }
    out.sort();
    out
}

pub(super) fn claim_rate_windows(
    from: NaiveDate,
    to: NaiveDate,
    source: ReferenceRateSource,
    single_annual_bps: RateBps,
    fallback_reference_bps: RateBps,
) -> Vec<RateWindow> {
    if source != ReferenceRateSource::StatutoryTable {
        return vec![RateWindow {
            end: to,
            annual_rate_bps: single_annual_bps,
        }];
    }
    let mut windows = Vec::new();
    let mut segment_start = from;
    for boundary in statutory_rate_boundaries_between(from, to) {
        let segment_rate =
            lookup_statutory_reference_rate(segment_start).unwrap_or(fallback_reference_bps);
        windows.push(RateWindow {
            end: boundary,
            annual_rate_bps: rate_from_reference(segment_rate),
        });
        segment_start = boundary;
    }
    let last_rate =
        lookup_statutory_reference_rate(segment_start).unwrap_or(fallback_reference_bps);
    windows.push(RateWindow {
        end: to,
        annual_rate_bps: rate_from_reference(last_rate),
    });
    windows
}

fn ms_to_date(unix_ms: i64) -> NaiveDate {
    Utc.timestamp_millis_opt(unix_ms)
        .single()
        .map(|dt| dt.date_naive())
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}

fn balance_events(invoice: &Invoice) -> Result<Vec<BalanceEvent>, InvoiceError> {
    let mut events = Vec::new();
    for p in &invoice.payments {
        events.push(BalanceEvent {
            date: ms_to_date(p.unix_ms),
            delta_minor: -p.amount_minor,
        });
    }
    for c in &invoice.credits {
        events.push(BalanceEvent {
            date: ms_to_date(c.unix_ms),
            delta_minor: -c.gross_minor,
        });
    }
    Ok(events)
}

fn open_balance_as_of(
    invoice: &Invoice,
    events: &[BalanceEvent],
    as_of: NaiveDate,
) -> Result<i64, InvoiceError> {
    let mut balance = invoice.gross_minor()?;
    for e in events {
        if e.date <= as_of {
            balance = balance
                .checked_add(e.delta_minor)
                .ok_or(InvoiceError::Overflow)?;
        }
    }
    Ok(balance)
}

fn window_segments(
    invoice: &Invoice,
    events: &[BalanceEvent],
    from: NaiveDate,
    to: NaiveDate,
    annual_rate_bps: RateBps,
) -> Result<Vec<InterestSegment>, InvoiceError> {
    if diff_days(from, to) <= 0 {
        return Ok(Vec::new());
    }
    let mut breakpoints = vec![from];
    for e in events {
        if e.date > from && e.date < to {
            breakpoints.push(e.date);
        }
    }
    breakpoints.push(to);
    breakpoints.sort();
    breakpoints.dedup();

    let mut out = Vec::new();
    for pair in breakpoints.windows(2) {
        let start = pair[0];
        let end = pair[1];
        let days = diff_days(start, end);
        if days <= 0 {
            continue;
        }
        let principal = open_balance_as_of(invoice, events, start)?;
        if principal > 0 {
            out.push(InterestSegment {
                principal_minor: principal,
                annual_rate_bps,
                days,
            });
        }
    }
    Ok(out)
}

pub(super) fn accrue_windows(
    invoice: &Invoice,
    from: Option<NaiveDate>,
    windows: &[RateWindow],
) -> Result<i64, InvoiceError> {
    let events = balance_events(invoice)?;
    let mut segments = Vec::new();
    let mut start = from;
    for w in windows {
        let Some(from_date) = start else {
            break;
        };
        segments.extend(window_segments(
            invoice,
            &events,
            from_date,
            w.end,
            w.annual_rate_bps,
        )?);
        start = Some(w.end);
    }
    cumulative_interest_minor(&segments)
}

pub(super) fn principal_open_minor(
    invoice: &Invoice,
    as_of: NaiveDate,
) -> Result<i64, InvoiceError> {
    open_balance_as_of(invoice, &balance_events(invoice)?, as_of)
}
