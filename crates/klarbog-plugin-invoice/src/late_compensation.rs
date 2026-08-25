//! Statutory fixed compensation (fast kompensation) — porteret fra originalens
//! `invoice-compensation.ts` (DK-INVOICE-LATE-COMPENSATION-001).

use crate::draft::{self, InvoiceConfig};
use crate::due_date::{assess_overdue, format_iso_date, parse_iso_date};
use crate::{Invoice, InvoiceError, InvoiceId};
use chrono::NaiveDate;
use klarbog_plugin_crm::{get_party, PartyKind};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const RULE_ID: &str = "DK-INVOICE-LATE-COMPENSATION-001";
pub const REGISTER_RULE_ID: &str = "DK-INVOICE-LATE-COMPENSATION-REGISTER-001";
pub const BOOKKEEPING_RULE_ID: &str = "DK-INVOICE-LATE-COMPENSATION-BOOKKEEPING-001";

/// Statutory fixed compensation: 310 DKK (31000 øre).
pub const STATUTORY_COMPENSATION_MINOR: i64 = 31_000;
pub const STATUTORY_COMPENSATION_START_DATE: &str = "2013-03-01";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceCompensationClaim {
    pub claim_date: String,
    pub amount_minor: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_journal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LateCompensationCalculation {
    pub as_of_date: String,
    pub effective_due_date: Option<String>,
    pub overdue_days: u32,
    pub principal_open_minor: i64,
    pub is_commercial_transaction: bool,
    pub eligible: bool,
    pub compensation_amount_minor: i64,
    pub reason: String,
}

fn normalize_amount(amount_minor: Option<i64>) -> Result<i64, InvoiceError> {
    let amount = amount_minor.unwrap_or(STATUTORY_COMPENSATION_MINOR);
    if amount <= 0 {
        return Err(InvoiceError::InvalidCompensationAmount);
    }
    if amount > STATUTORY_COMPENSATION_MINOR {
        return Err(InvoiceError::CompensationExceedsStatutoryMax {
            amount_minor: amount,
            max_minor: STATUTORY_COMPENSATION_MINOR,
        });
    }
    Ok(amount)
}

/// Commercial buyer for renteloven § 9a: business party in CRM (ADR-020).
pub fn buyer_is_commercial_party(company: &Path, invoice: &Invoice) -> Result<bool, InvoiceError> {
    let party = get_party(company, &invoice.party_id)?
        .ok_or_else(|| InvoiceError::PartyNotFound(invoice.party_id.to_string()))?;
    Ok(party.kind == PartyKind::Business)
}

pub fn calculate_late_compensation(
    company: &Path,
    invoice: &Invoice,
    as_of: NaiveDate,
    compensation_amount_minor: Option<i64>,
) -> Result<LateCompensationCalculation, InvoiceError> {
    let amount = normalize_amount(compensation_amount_minor)?;
    let is_commercial = buyer_is_commercial_party(company, invoice)?;
    let collectible = invoice.collectible_open_minor()?;
    let due = assess_overdue(
        invoice.issue_date.as_deref(),
        invoice.due_date.as_deref(),
        collectible,
        as_of,
    )?;
    let as_of_s = format_iso_date(as_of);
    let issue = invoice.issue_date.as_deref().unwrap_or(as_of_s.as_str());
    let covered = issue >= STATUTORY_COMPENSATION_START_DATE;
    let eligible =
        is_commercial && collectible > 0 && due.is_overdue && due.overdue_days > 0 && covered;

    let reason = if !is_commercial {
        "buyer is not a commercial party (CRM kind must be business)".into()
    } else if collectible <= 0 {
        "invoice has no collectible open balance".into()
    } else if !due.is_overdue || due.overdue_days == 0 {
        "invoice is not overdue as of the requested date".into()
    } else if !covered {
        format!(
            "invoice predates statutory compensation start date {STATUTORY_COMPENSATION_START_DATE}"
        )
    } else {
        "eligible".into()
    };

    Ok(LateCompensationCalculation {
        as_of_date: format_iso_date(as_of),
        effective_due_date: due.effective_due_date,
        overdue_days: due.overdue_days,
        principal_open_minor: collectible,
        is_commercial_transaction: is_commercial,
        eligible,
        compensation_amount_minor: if eligible { amount } else { 0 },
        reason,
    })
}

pub fn register_invoice_compensation(
    company: &Path,
    id: &InvoiceId,
    as_of: NaiveDate,
    compensation_amount_minor: Option<i64>,
    note: Option<String>,
) -> Result<(Invoice, LateCompensationCalculation), InvoiceError> {
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
    if !invoice.compensation_claims.is_empty() {
        return Err(InvoiceError::CompensationAlreadyRegistered);
    }

    let calc = calculate_late_compensation(company, invoice, as_of, compensation_amount_minor)?;
    if !calc.eligible || calc.compensation_amount_minor <= 0 {
        return Err(InvoiceError::CompensationNotEligible(calc.reason));
    }

    let claim = InvoiceCompensationClaim {
        claim_date: calc.as_of_date.clone(),
        amount_minor: calc.compensation_amount_minor,
        note,
        posted_journal_id: None,
    };
    invoice.compensation_claims.push(claim.clone());
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    crate::claim_ledger::dual_write_compensation(company, id, &claim)?;
    Ok((updated, calc))
}

pub fn oldest_unposted_compensation_claim(
    invoice: &Invoice,
) -> Option<(usize, &InvoiceCompensationClaim)> {
    invoice
        .compensation_claims
        .iter()
        .enumerate()
        .find(|(_, c)| c.posted_journal_id.is_none())
}

pub fn compensation_post_journal_suggestion(
    invoice: &Invoice,
    claim: &InvoiceCompensationClaim,
    actor: &klarbog_types::Actor,
    cfg: &InvoiceConfig,
) -> Result<klarbog_journal::JournalEntry, InvoiceError> {
    if claim.posted_journal_id.is_some() {
        return Err(InvoiceError::CompensationClaimAlreadyPosted);
    }
    if claim.amount_minor <= 0 {
        return Err(InvoiceError::InvalidCompensationAmount);
    }
    invoice.validate_lines()?;
    let accounts = crate::claim_posting_accounts::resolve_claim_posting_accounts(cfg)?;
    let currency = invoice.lines[0].currency.clone();
    let party = Some(invoice.party_id.clone());
    let invoice_no = invoice.invoice_no.as_deref().unwrap_or("?");
    let memo = format!(
        "invoice:{}:compensation:{} · {}",
        invoice.id, claim.claim_date, invoice_no
    );
    let amount = klarbog_types::MinorAmount::from_minor(claim.amount_minor);
    let legs = vec![
        draft::leg(
            &accounts.receivable_account,
            klarbog_journal::Direction::Debit,
            amount,
            &currency,
            party.clone(),
        ),
        draft::leg(
            &accounts.income_account,
            klarbog_journal::Direction::Credit,
            amount,
            &currency,
            party,
        ),
    ];
    let as_of = parse_iso_date(&claim.claim_date)?;
    let entry = klarbog_journal::JournalEntry {
        memo,
        legs,
        as_of: as_of.and_hms_opt(12, 0, 0).unwrap().and_utc(),
        actor: actor.clone(),
    };
    entry.validate().map_err(InvoiceError::Journal)?;
    Ok(entry)
}

pub fn mark_compensation_posted(
    company: &Path,
    id: &InvoiceId,
    claim_date: &str,
    journal_entry_id: &str,
) -> Result<Invoice, InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    let claim = invoice
        .compensation_claims
        .iter_mut()
        .find(|c| c.claim_date == claim_date)
        .ok_or_else(|| InvoiceError::CompensationClaimNotFound(claim_date.to_string()))?;
    if claim.posted_journal_id.is_some() {
        return Err(InvoiceError::CompensationClaimAlreadyPosted);
    }
    claim.posted_journal_id = Some(journal_entry_id.to_string());
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    crate::claim_ledger::dual_write_compensation_posting(
        company,
        id,
        claim_date,
        journal_entry_id,
    )?;
    Ok(updated)
}

pub fn total_compensation_minor(invoice: &Invoice) -> Result<i64, InvoiceError> {
    invoice.compensation_claims.iter().try_fold(0i64, |acc, c| {
        acc.checked_add(c.amount_minor)
            .ok_or(InvoiceError::Overflow)
    })
}
