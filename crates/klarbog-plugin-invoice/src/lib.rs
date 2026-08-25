//! Invoice plugin — drafts in `invoices.json` + journal suggestions with party_id.
//! No journal-write capability (ADR-004).

mod claim_ledger;
#[cfg(test)]
mod claim_ledger_tests;
mod credit;
mod draft;
pub mod due_date;
pub mod email;
mod email_ledger;
mod email_pdf;
mod error;
mod invoice_numbers;
mod issue;
mod late_compensation;
#[cfg(test)]
mod late_compensation_tests;
mod late_interest;
#[cfg(test)]
mod late_interest_tests;
mod lifecycle;
mod reminders;
#[cfg(test)]
mod reminders_tests;
mod sequences;
mod status;
mod store;
#[cfg(test)]
mod test_fixtures;

pub use credit::{credit_amounts_from_entry, credit_journal_suggestion};
pub use draft::{
    issue_journal_suggestion, journal_suggestion, payment_journal_suggestion,
    payment_journal_suggestion_amount, InvoiceConfig,
};
pub use due_date::{
    assess_overdue, effective_due_date, format_iso_date, parse_iso_date, InvoiceDueAssessment,
    STATUTORY_PAYMENT_TERM_DAYS,
};
pub use email::{
    deterministic_message_id, looks_like_email, read_send_log, send_invoice_email, EmailKind,
    EmailSendLogRow, SendInvoiceEmailOutcome, EMAIL_SEND_LOG, RULE_ID as EMAIL_DELIVERY_RULE_ID,
};
pub use invoice_numbers::{
    invoice_no_from_memo, peek_invoice_number, reserve_invoice_number, resolve_invoice_number,
    validate_manual_invoice_number_scope,
};
pub use issue::record_issue;
pub use late_compensation::{
    buyer_is_commercial_party, calculate_late_compensation, compensation_post_journal_suggestion,
    mark_compensation_posted, oldest_unposted_compensation_claim, register_invoice_compensation,
    total_compensation_minor, InvoiceCompensationClaim, LateCompensationCalculation,
    BOOKKEEPING_RULE_ID as LATE_COMPENSATION_BOOKKEEPING_RULE_ID,
    REGISTER_RULE_ID as LATE_COMPENSATION_REGISTER_RULE_ID, RULE_ID as LATE_COMPENSATION_RULE_ID,
    STATUTORY_COMPENSATION_MINOR, STATUTORY_COMPENSATION_START_DATE,
};
pub use late_interest::{
    calculate_late_interest, claim_open_balance_minor, cumulative_interest_minor,
    interest_post_journal_suggestion, lookup_statutory_reference_rate, mark_interest_claim_posted,
    oldest_unposted_interest_claim, register_late_interest, resolve_unposted_interest_claim,
    total_interest_claims_minor, InvoiceInterestClaim, LateInterestCalculation,
    ReferenceRateSource, BOOKKEEPING_RULE_ID as LATE_INTEREST_BOOKKEEPING_RULE_ID,
    REGISTER_RULE_ID, RULE_ID as LATE_INTEREST_RULE_ID, STATUTORY_SURCHARGE_BPS,
};
pub use lifecycle::{
    mark_paid_preview, mark_part_paid_preview, patch_status, record_credit_note, record_payment,
};
pub use reminders::{
    mark_reminder_posted, oldest_unposted_reminder, register_invoice_reminder,
    reminder_post_journal_suggestion, resolve_unposted_reminder, rollback_unposted_reminder,
    total_reminder_fees_minor, InvoiceReminder, RegisterInvoiceReminderResult,
    BOOKKEEPING_RULE_ID as REMINDER_BOOKKEEPING_RULE_ID, MAX_REMINDERS_PER_CLAIM,
    MAX_REMINDER_FEE_MINOR, MIN_DAYS_BETWEEN_REMINDERS, RULE_ID as REMINDER_FEE_RULE_ID,
};
pub use sequences::{
    credit_note_no_from_memo, credit_reason_from_memo, peek_credit_note_number,
    reserve_credit_note_number, resolve_credit_note_number,
    validate_manual_credit_note_number_scope, SEQUENCES_FILENAME,
};
pub use status::InvoiceStatus;
pub use store::{
    create_draft, create_draft_from_new, create_draft_from_new_with_due, get_invoice,
    list_invoices, NewLine, INVOICES_FILENAME,
};

use klarbog_plugin::{Capability, Plugin};
use klarbog_types::{Currency, PartyId};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub use error::InvoiceError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceKind {
    Sale,
    Purchase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceId(String);

impl InvoiceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn generate() -> Self {
        Self(format!("inv_{}", uuid::Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InvoiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub description: String,
    pub amount_minor: i64,
    pub currency: Currency,
}

/// One recorded payment against an invoice (company `invoices.json` ledger).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoicePayment {
    pub unix_ms: i64,
    pub amount_minor: i64,
    pub currency: Currency,
}

/// VAT captured at creation from the party kind (ADR-020): private parties
/// are invoiced gross-inclusive, business parties net-exclusive. Frozen on
/// the invoice so a later party-kind change never rewrites existing drafts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceVat {
    pub net_minor: i64,
    pub vat_minor: i64,
    pub gross_minor: i64,
    /// Basis points (2500 = 25 %).
    pub rate_bps: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceCredit {
    pub unix_ms: i64,
    pub credit_note_no: String,
    pub gross_minor: i64,
    pub net_minor: i64,
    pub vat_minor: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invoice {
    pub id: InvoiceId,
    pub party_id: PartyId,
    pub kind: InvoiceKind,
    pub lines: Vec<InvoiceLine>,
    #[serde(default)]
    pub status: InvoiceStatus,
    /// Cumulative payment ledger; remaining = gross − sum(amount_minor).
    #[serde(default)]
    pub payments: Vec<InvoicePayment>,
    /// Cumulative credit-note ledger (DK-CREDIT-NOTE-001). Sum of
    /// `gross_minor` must never exceed original gross.
    #[serde(default)]
    pub credits: Vec<InvoiceCredit>,
    /// `None` = legacy invoice (pre ADR-020) → booked without VAT legs.
    #[serde(default)]
    pub vat: Option<InvoiceVat>,
    /// Seneste kreditnota-nummer — historik ligger i `credits`.
    #[serde(default)]
    pub credit_note_no: Option<String>,
    /// Udstedelsesdato (YYYY-MM-DD) — sat ved bogføring/send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_date: Option<String>,
    /// Eksplicit forfaldsdato (YYYY-MM-DD); ellers +30 dage fra issue_date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Fortløbende fakturanummer ved udstedelse (DK-INVOICE-ISSUE-001).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_no: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_sha256: Option<String>,
    /// Registrerede morarentekrav (DK-INVOICE-LATE-INTEREST-REGISTER-001).
    #[serde(default)]
    pub interest_claims: Vec<InvoiceInterestClaim>,
    /// Registrerede rykkergebyrer (DK-INVOICE-REMINDER-FEE-001).
    #[serde(default)]
    pub reminders: Vec<InvoiceReminder>,
    /// Registreret fast kompensation (DK-INVOICE-LATE-COMPENSATION-REGISTER-001).
    #[serde(default)]
    pub compensation_claims: Vec<InvoiceCompensationClaim>,
}

impl Invoice {
    pub fn validate_lines(&self) -> Result<(), InvoiceError> {
        if self.lines.is_empty() {
            return Err(InvoiceError::NoLines);
        }
        let currency = &self.lines[0].currency;
        for line in &self.lines {
            if line.description.trim().is_empty() {
                return Err(InvoiceError::EmptyDescription);
            }
            if line.amount_minor <= 0 {
                return Err(InvoiceError::NonPositiveAmount);
            }
            if &line.currency != currency {
                return Err(InvoiceError::MixedCurrency);
            }
        }
        Ok(())
    }

    pub fn total_minor(&self) -> Result<i64, InvoiceError> {
        self.validate_lines()?;
        self.lines.iter().try_fold(0i64, |acc, l| {
            acc.checked_add(l.amount_minor)
                .ok_or(InvoiceError::Overflow)
        })
    }

    /// Payable amount incl. VAT — what hits the bank. Legacy invoices
    /// (`vat: None`) fall back to the line total.
    pub fn gross_minor(&self) -> Result<i64, InvoiceError> {
        match &self.vat {
            Some(v) => Ok(v.gross_minor),
            None => self.total_minor(),
        }
    }

    pub fn paid_minor(&self) -> Result<i64, InvoiceError> {
        self.payments.iter().try_fold(0i64, |acc, p| {
            if p.amount_minor <= 0 {
                return Err(InvoiceError::NonPositiveAmount);
            }
            acc.checked_add(p.amount_minor)
                .ok_or(InvoiceError::Overflow)
        })
    }

    /// Open balance: `gross_minor − paid_minor` (i64 only; fail-closed on overflow).
    pub fn remaining_minor(&self) -> Result<i64, InvoiceError> {
        let gross = self.gross_minor()?;
        let paid = self.paid_minor()?;
        gross.checked_sub(paid).ok_or(InvoiceError::Overflow)
    }

    /// Inddriveligt hovedstol: brutto − krediteret − betalt (DK-INVOICE-DUE-DATE-001).
    pub fn collectible_open_minor(&self) -> Result<i64, InvoiceError> {
        let gross = self.gross_minor()?;
        let paid = self.paid_minor()?;
        let credited = self.credited_gross_minor()?;
        gross
            .checked_sub(credited)
            .and_then(|v| v.checked_sub(paid))
            .ok_or(InvoiceError::Overflow)
    }

    pub fn due_assessment(
        &self,
        as_of: chrono::NaiveDate,
    ) -> Result<InvoiceDueAssessment, InvoiceError> {
        assess_overdue(
            self.issue_date.as_deref(),
            self.due_date.as_deref(),
            self.collectible_open_minor()?,
            as_of,
        )
    }

    pub fn credited_gross_minor(&self) -> Result<i64, InvoiceError> {
        self.credits.iter().try_fold(0i64, |acc, c| {
            if c.gross_minor <= 0 {
                return Err(InvoiceError::NonPositiveAmount);
            }
            acc.checked_add(c.gross_minor).ok_or(InvoiceError::Overflow)
        })
    }

    pub fn credited_vat_minor(&self) -> Result<i64, InvoiceError> {
        self.credits.iter().try_fold(0i64, |acc, c| {
            acc.checked_add(c.vat_minor).ok_or(InvoiceError::Overflow)
        })
    }

    pub fn credited_net_minor(&self) -> Result<i64, InvoiceError> {
        self.credits.iter().try_fold(0i64, |acc, c| {
            acc.checked_add(c.net_minor).ok_or(InvoiceError::Overflow)
        })
    }

    /// Gross still available to credit (original − credited so far).
    pub fn creditable_remaining_minor(&self) -> Result<i64, InvoiceError> {
        let gross = self.gross_minor()?;
        let credited = self.credited_gross_minor()?;
        gross.checked_sub(credited).ok_or(InvoiceError::Overflow)
    }
}

pub struct InvoicePlugin;

impl Default for InvoicePlugin {
    fn default() -> Self {
        Self
    }
}

impl InvoicePlugin {
    pub fn create(
        &self,
        company: &Path,
        party_id: PartyId,
        kind: InvoiceKind,
        lines: Vec<NewLine>,
    ) -> Result<Invoice, InvoiceError> {
        create_draft_from_new(company, party_id, kind, lines)
    }

    pub fn list(&self, company: &Path) -> Result<Vec<Invoice>, InvoiceError> {
        list_invoices(company)
    }

    pub fn get(&self, company: &Path, id: &InvoiceId) -> Result<Option<Invoice>, InvoiceError> {
        get_invoice(company, id)
    }
}

impl Plugin for InvoicePlugin {
    fn id(&self) -> &'static str {
        "invoice"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::Read, Capability::CrmWrite]
    }
}

#[cfg(test)]
mod tests;
