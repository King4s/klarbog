//! Invoice plugin error types.

use crate::InvoiceStatus;
use klarbog_types::MoneyError;
use thiserror::Error;

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
    #[error("invalid invoice number: {0}")]
    BadInvoiceNumber(String),
    #[error("manual invoice number {number} does not match current fiscal scope {scope}")]
    ManualInvoiceScopeMismatch { number: String, scope: String },
    #[error("manual credit note number {number} does not match current fiscal scope {scope}")]
    ManualCreditNoteScopeMismatch { number: String, scope: String },
    #[error("due date {due_date} cannot be earlier than issue date {issue_date}")]
    DueBeforeIssue {
        due_date: String,
        issue_date: String,
    },
    #[error("due date must be YYYY-MM-DD: {0}")]
    InvalidDueDate(String),
    #[error("no statutory reference rate tabled for {0}")]
    NoStatutoryReferenceRate(String),
    #[error("reference rate must not be negative")]
    InvalidReferenceRate,
    #[error("late interest must be positive before it can be registered")]
    NoInterestToRegister,
    #[error("late interest already registered for {claim_date} at reference rate {reference_rate_bps} bps")]
    DuplicateInterestClaim {
        claim_date: String,
        reference_rate_bps: i64,
    },
    #[error("interest claim not found for date {0}")]
    InterestClaimNotFound(String),
    #[error("ambiguous interest claim for date {claim_date}; pass reference_rate_bps")]
    AmbiguousInterestClaim { claim_date: String },
    #[error("interest claim is already posted")]
    InterestClaimAlreadyPosted,
    #[error("only DKK issued invoices are supported in the current reminder flow")]
    NonDkkInvoice(String),
    #[error("reminder fee must be positive")]
    InvalidReminderFee,
    #[error("reminder fee {fee_minor} exceeds statutory maximum {max_minor} øre")]
    ReminderFeeExceedsStatutoryMax { fee_minor: i64, max_minor: i64 },
    #[error("invoice must be overdue with positive open balance on reminder date")]
    NotOverdueForReminder,
    #[error("cannot register more than {max} reminder fees for the same invoice claim")]
    MaxRemindersReached { max: usize },
    #[error("reminder date must be at least {min_days} days after the previous reminder on {previous_date}")]
    ReminderTooSoon {
        previous_date: String,
        min_days: i64,
    },
    #[error("reminder not found for date {0}")]
    ReminderNotFound(String),
    #[error("reminder is already posted")]
    ReminderAlreadyPosted,
    #[error("compensation amount must be positive")]
    InvalidCompensationAmount,
    #[error("compensation amount {amount_minor} exceeds statutory maximum {max_minor} øre")]
    CompensationExceedsStatutoryMax { amount_minor: i64, max_minor: i64 },
    #[error("invoice is not eligible for compensation: {0}")]
    CompensationNotEligible(String),
    #[error("invoice already has a registered compensation claim")]
    CompensationAlreadyRegistered,
    #[error("compensation claim not found for date {0}")]
    CompensationClaimNotFound(String),
    #[error("compensation claim is already posted")]
    CompensationClaimAlreadyPosted,
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
    #[error("invoice must be sent before email delivery")]
    NotSentForEmail,
    #[error("issued invoice document is missing")]
    MissingIssuedDocument,
    #[error("no recipient email for invoice {0}")]
    MissingRecipientEmail(String),
    #[error("invalid recipient email: {0}")]
    InvalidRecipientEmail(String),
    #[error("email send failed: {0}")]
    EmailSendFailed(String),
    #[error("non-DKK invoice {0} requires fx_rate_to_dkk_micro")]
    MissingFxRate(String),
    #[error("fx_rate_to_dkk_micro must be positive")]
    InvalidFxRate,
    #[error("invalid claim posting account: {0}")]
    InvalidClaimAccount(String),
    #[error(transparent)]
    Store(#[from] klarbog_store_sqlite::StoreError),
}
