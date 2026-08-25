//! Statutory reminder fees (rykkergebyr) — porteret fra originalens
//! `invoice-reminders.ts` (DK-INVOICE-REMINDER-FEE-001).

use crate::draft::{self, InvoiceConfig};
use crate::due_date::{assess_overdue, diff_days, format_iso_date, parse_iso_date};
use crate::{Invoice, InvoiceError, InvoiceId};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const RULE_ID: &str = "DK-INVOICE-REMINDER-FEE-001";
pub const BOOKKEEPING_RULE_ID: &str = "DK-INVOICE-REMINDER-FEE-BOOKKEEPING-001";

/// Statutory maximum reminder fee: 100 DKK (10000 øre).
pub const MAX_REMINDER_FEE_MINOR: i64 = 10_000;
pub const MAX_REMINDERS_PER_CLAIM: usize = 3;
pub const MIN_DAYS_BETWEEN_REMINDERS: i64 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceReminder {
    pub reminder_date: String,
    pub fee_amount_minor: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_journal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterInvoiceReminderResult {
    pub reminder_sequence: usize,
    pub reminder_date: String,
    pub fee_amount_minor: i64,
    pub total_reminder_fees_minor: i64,
}

fn ensure_dkk(invoice: &Invoice) -> Result<(), InvoiceError> {
    invoice.validate_lines()?;
    let currency = invoice.lines[0].currency.as_str();
    if currency != "DKK" {
        return Err(InvoiceError::NonDkkInvoice(currency.to_string()));
    }
    Ok(())
}

fn normalize_fee(fee_amount_minor: Option<i64>) -> Result<i64, InvoiceError> {
    let fee = fee_amount_minor.unwrap_or(MAX_REMINDER_FEE_MINOR);
    if fee <= 0 {
        return Err(InvoiceError::InvalidReminderFee);
    }
    if fee > MAX_REMINDER_FEE_MINOR {
        return Err(InvoiceError::ReminderFeeExceedsStatutoryMax {
            fee_minor: fee,
            max_minor: MAX_REMINDER_FEE_MINOR,
        });
    }
    Ok(fee)
}

pub fn register_invoice_reminder(
    company: &Path,
    id: &InvoiceId,
    reminder_date: NaiveDate,
    fee_amount_minor: Option<i64>,
    note: Option<String>,
) -> Result<(Invoice, RegisterInvoiceReminderResult), InvoiceError> {
    let fee = normalize_fee(fee_amount_minor)?;
    let reminder_s = format_iso_date(reminder_date);

    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;

    if !invoice.status.allows_reminder() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: crate::InvoiceStatus::Sent,
        });
    }
    ensure_dkk(invoice)?;

    let collectible = invoice.collectible_open_minor()?;
    let due = assess_overdue(
        invoice.issue_date.as_deref(),
        invoice.due_date.as_deref(),
        collectible,
        reminder_date,
    )?;
    if !due.is_overdue || collectible <= 0 {
        return Err(InvoiceError::NotOverdueForReminder);
    }

    if invoice.reminders.len() >= MAX_REMINDERS_PER_CLAIM {
        return Err(InvoiceError::MaxRemindersReached {
            max: MAX_REMINDERS_PER_CLAIM,
        });
    }

    if let Some(latest) = invoice.reminders.last() {
        let latest_date = parse_iso_date(&latest.reminder_date)?;
        let days_since = diff_days(latest_date, reminder_date);
        if days_since < MIN_DAYS_BETWEEN_REMINDERS {
            return Err(InvoiceError::ReminderTooSoon {
                previous_date: latest.reminder_date.clone(),
                min_days: MIN_DAYS_BETWEEN_REMINDERS,
            });
        }
    }

    let prior_total = total_reminder_fees_minor(invoice)?;
    let reminder = InvoiceReminder {
        reminder_date: reminder_s.clone(),
        fee_amount_minor: fee,
        note,
        posted_journal_id: None,
    };
    invoice.reminders.push(reminder.clone());
    let result = RegisterInvoiceReminderResult {
        reminder_sequence: invoice.reminders.len(),
        reminder_date: reminder_s,
        fee_amount_minor: fee,
        total_reminder_fees_minor: prior_total.checked_add(fee).ok_or(InvoiceError::Overflow)?,
    };
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    crate::claim_ledger::dual_write_reminder(company, id, &reminder)?;
    Ok((updated, result))
}

/// Remove an unposted reminder row (rollback when compound send fails after register).
pub fn rollback_unposted_reminder(
    company: &Path,
    id: &InvoiceId,
    reminder_date: &str,
) -> Result<(), InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    let Some(pos) = invoice
        .reminders
        .iter()
        .position(|r| r.reminder_date == reminder_date && r.posted_journal_id.is_none())
    else {
        return Ok(());
    };
    invoice.reminders.remove(pos);
    crate::store::save(company, &file)?;
    Ok(())
}

pub fn oldest_unposted_reminder(invoice: &Invoice) -> Option<(usize, &InvoiceReminder)> {
    invoice
        .reminders
        .iter()
        .enumerate()
        .find(|(_, r)| r.posted_journal_id.is_none())
}

/// Resolve which unposted reminder to book.
///
/// - `reminder_date` omitted → oldest unposted (backward compatible).
/// - `reminder_date` set → that reminder; fail-closed if missing or already posted.
/// - `reminder_sequence` (1-based) may be used instead of / with date; both must agree.
pub fn resolve_unposted_reminder<'a>(
    invoice: &'a Invoice,
    reminder_date: Option<&str>,
    reminder_sequence: Option<usize>,
) -> Result<(usize, &'a InvoiceReminder), InvoiceError> {
    if reminder_date.is_none() && reminder_sequence.is_none() {
        return oldest_unposted_reminder(invoice)
            .ok_or_else(|| InvoiceError::ReminderNotFound("unposted".into()));
    }
    let by_seq = reminder_sequence
        .map(|seq| {
            if seq == 0 || seq > invoice.reminders.len() {
                return Err(InvoiceError::ReminderNotFound(format!("sequence {seq}")));
            }
            Ok(seq - 1)
        })
        .transpose()?;
    let by_date = reminder_date
        .map(|date| {
            invoice
                .reminders
                .iter()
                .position(|r| r.reminder_date == date)
                .ok_or_else(|| InvoiceError::ReminderNotFound(date.into()))
        })
        .transpose()?;
    let idx = match (by_date, by_seq) {
        (Some(d), Some(s)) if d != s => {
            return Err(InvoiceError::ReminderNotFound(
                "date/sequence mismatch".to_string(),
            ));
        }
        (Some(d), _) => d,
        (None, Some(s)) => s,
        (None, None) => unreachable!(),
    };
    let reminder = &invoice.reminders[idx];
    if reminder.posted_journal_id.is_some() {
        return Err(InvoiceError::ReminderAlreadyPosted);
    }
    Ok((idx, reminder))
}

pub fn reminder_post_journal_suggestion(
    invoice: &Invoice,
    reminder: &InvoiceReminder,
    actor: &klarbog_types::Actor,
    cfg: &InvoiceConfig,
) -> Result<klarbog_journal::JournalEntry, InvoiceError> {
    if reminder.posted_journal_id.is_some() {
        return Err(InvoiceError::ReminderAlreadyPosted);
    }
    if reminder.fee_amount_minor <= 0 {
        return Err(InvoiceError::InvalidReminderFee);
    }
    ensure_dkk(invoice)?;
    let currency = invoice.lines[0].currency.clone();
    let party = Some(invoice.party_id.clone());
    let invoice_no = invoice.invoice_no.as_deref().unwrap_or("?");
    let memo = format!(
        "invoice:{}:reminder:{} · {}",
        invoice.id, reminder.reminder_date, invoice_no
    );
    let amount = klarbog_types::MinorAmount::from_minor(reminder.fee_amount_minor);
    let legs = vec![
        draft::leg(
            &cfg.ar_account,
            klarbog_journal::Direction::Debit,
            amount,
            &currency,
            party.clone(),
        ),
        draft::leg(
            &cfg.interest_income_account,
            klarbog_journal::Direction::Credit,
            amount,
            &currency,
            party,
        ),
    ];
    let as_of = parse_iso_date(&reminder.reminder_date)?;
    let entry = klarbog_journal::JournalEntry {
        memo,
        legs,
        as_of: as_of.and_hms_opt(12, 0, 0).unwrap().and_utc(),
        actor: actor.clone(),
    };
    entry.validate().map_err(InvoiceError::Journal)?;
    Ok(entry)
}

pub fn mark_reminder_posted(
    company: &Path,
    id: &InvoiceId,
    reminder_date: &str,
    journal_entry_id: &str,
) -> Result<Invoice, InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    let reminder = invoice
        .reminders
        .iter_mut()
        .find(|r| r.reminder_date == reminder_date)
        .ok_or_else(|| InvoiceError::ReminderNotFound(reminder_date.to_string()))?;
    if reminder.posted_journal_id.is_some() {
        return Err(InvoiceError::ReminderAlreadyPosted);
    }
    reminder.posted_journal_id = Some(journal_entry_id.to_string());
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    crate::claim_ledger::dual_write_reminder_posting(company, id, reminder_date, journal_entry_id)?;
    Ok(updated)
}

pub fn total_reminder_fees_minor(invoice: &Invoice) -> Result<i64, InvoiceError> {
    invoice.reminders.iter().try_fold(0i64, |acc, r| {
        acc.checked_add(r.fee_amount_minor)
            .ok_or(InvoiceError::Overflow)
    })
}
