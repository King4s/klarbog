//! Invoice plugin — drafts in `invoices.json` + journal suggestions with party_id.
//! No journal-write capability (ADR-004).

mod credit;
mod draft;
mod lifecycle;
mod sequences;
mod status;
mod store;
#[cfg(test)]
mod test_fixtures;

pub use credit::{credit_amounts_from_entry, credit_journal_suggestion};
pub use draft::{
    journal_suggestion, payment_journal_suggestion, payment_journal_suggestion_amount,
    InvoiceConfig,
};
pub use lifecycle::{
    mark_paid_preview, mark_part_paid_preview, patch_status, record_credit_note, record_payment,
};
pub use sequences::{
    credit_note_no_from_memo, peek_credit_note_number, reserve_credit_note_number,
    SEQUENCES_FILENAME,
};
pub use status::InvoiceStatus;
pub use store::{
    create_draft, create_draft_from_new, get_invoice, list_invoices, NewLine, INVOICES_FILENAME,
};

use klarbog_plugin::{Capability, Plugin};
use klarbog_types::{Currency, MoneyError, PartyId};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum InvoiceError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("party not found: {0}")]
    PartyNotFound(String),
    #[error("invoice not found: {0}")]
    NotFound(String),
    #[error("invoice has no lines")]
    NoLines,
    #[error("line description must not be empty")]
    EmptyDescription,
    #[error("line amount must be positive")]
    NonPositiveAmount,
    #[error("partial amount {amount_minor} must be > 0 and < remaining {remaining_minor}")]
    InvalidPartialAmount {
        amount_minor: i64,
        remaining_minor: i64,
    },
    #[error("payment {amount_minor} exceeds remaining {remaining_minor}")]
    Overpay {
        amount_minor: i64,
        remaining_minor: i64,
    },
    #[error("cannot mark paid: remaining balance is 0")]
    NothingRemaining,
    #[error("cannot credit: invoice has recorded payments")]
    CreditWithPayments,
    #[error("credit note reason is required")]
    MissingCreditReason,
    #[error("nothing left to credit")]
    NothingCreditable,
    #[error("credit amount {amount_minor} must be positive")]
    CreditAmountInvalid { amount_minor: i64 },
    #[error("credit amount {amount_minor} exceeds remaining creditable {remaining_minor}")]
    CreditExceedsRemaining {
        amount_minor: i64,
        remaining_minor: i64,
    },
    #[error("sequence conflict: requested {requested} but next is {expected}")]
    SequenceConflict { requested: u32, expected: u32 },
    #[error("invalid credit note number: {0}")]
    BadCreditNoteNumber(String),
    #[error("mixed currencies in one invoice")]
    MixedCurrency,
    #[error("overflow")]
    Overflow,
    #[error("invalid status transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: InvoiceStatus,
        to: InvoiceStatus,
    },
    #[error(transparent)]
    Money(#[from] MoneyError),
    #[error(transparent)]
    Journal(#[from] klarbog_journal::JournalError),
    #[error(transparent)]
    Crm(#[from] klarbog_plugin_crm::CrmError),
    #[error("vat: {0}")]
    Vat(#[from] klarbog_plugin_rules_dk::VatSplitError),
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
