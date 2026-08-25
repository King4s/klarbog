//! Posting helpers for late-interest claims (linegate split from late_interest.rs).

use super::InvoiceInterestClaim;
use crate::draft::{self, InvoiceConfig};
use crate::due_date::parse_iso_date;
use crate::{Invoice, InvoiceError, InvoiceId};
use std::path::Path;

pub fn oldest_unposted_interest_claim(invoice: &Invoice) -> Option<(usize, &InvoiceInterestClaim)> {
    invoice
        .interest_claims
        .iter()
        .enumerate()
        .find(|(_, c)| c.posted_journal_id.is_none())
}

/// Resolve which unposted interest claim to book.
///
/// - `claim_date` omitted → oldest unposted (backward compatible).
/// - `claim_date` set → that claim; fail-closed if missing or already posted.
/// - `reference_rate_bps` disambiguates when several claims share a date.
pub fn resolve_unposted_interest_claim<'a>(
    invoice: &'a Invoice,
    claim_date: Option<&str>,
    reference_rate_bps: Option<i64>,
) -> Result<(usize, &'a InvoiceInterestClaim), InvoiceError> {
    match claim_date {
        None => oldest_unposted_interest_claim(invoice)
            .ok_or_else(|| InvoiceError::InterestClaimNotFound("unposted".into())),
        Some(date) => {
            let matches: Vec<(usize, &InvoiceInterestClaim)> = invoice
                .interest_claims
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    c.claim_date == date
                        && reference_rate_bps.is_none_or(|r| c.reference_rate_bps == r)
                })
                .collect();
            match matches.as_slice() {
                [] => Err(InvoiceError::InterestClaimNotFound(date.into())),
                [(idx, claim)] => {
                    if claim.posted_journal_id.is_some() {
                        return Err(InvoiceError::InterestClaimAlreadyPosted);
                    }
                    Ok((*idx, *claim))
                }
                _ => Err(InvoiceError::AmbiguousInterestClaim {
                    claim_date: date.into(),
                }),
            }
        }
    }
}

pub fn interest_post_journal_suggestion(
    invoice: &Invoice,
    claim: &InvoiceInterestClaim,
    actor: &klarbog_types::Actor,
    cfg: &InvoiceConfig,
) -> Result<klarbog_journal::JournalEntry, InvoiceError> {
    if claim.posted_journal_id.is_some() {
        return Err(InvoiceError::InterestClaimAlreadyPosted);
    }
    if claim.amount_minor <= 0 {
        return Err(InvoiceError::NoInterestToRegister);
    }
    invoice.validate_lines()?;
    let currency = invoice.lines[0].currency.clone();
    let party = Some(invoice.party_id.clone());
    let invoice_no = invoice.invoice_no.as_deref().unwrap_or("?");
    // date@rate so commit can mark the exact claim when several share a date
    let memo = format!(
        "invoice:{}:interest:{}@{} · {}",
        invoice.id, claim.claim_date, claim.reference_rate_bps, invoice_no
    );
    let amount = klarbog_types::MinorAmount::from_minor(claim.amount_minor);
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

pub fn mark_interest_claim_posted(
    company: &Path,
    id: &InvoiceId,
    claim_date: &str,
    reference_rate_bps: Option<i64>,
    journal_entry_id: &str,
) -> Result<Invoice, InvoiceError> {
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    let matches: Vec<usize> = invoice
        .interest_claims
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.claim_date == claim_date
                && reference_rate_bps.is_none_or(|r| c.reference_rate_bps == r)
        })
        .map(|(i, _)| i)
        .collect();
    let idx = match matches.as_slice() {
        [] => return Err(InvoiceError::InterestClaimNotFound(claim_date.into())),
        [i] => *i,
        _ => {
            return Err(InvoiceError::AmbiguousInterestClaim {
                claim_date: claim_date.into(),
            })
        }
    };
    let claim = &mut invoice.interest_claims[idx];
    if claim.posted_journal_id.is_some() {
        return Err(InvoiceError::InterestClaimAlreadyPosted);
    }
    claim.posted_journal_id = Some(journal_entry_id.to_string());
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok(updated)
}
