//! Dual-write claim registration + posting links to company ledger SQLite.
//! invoices.json remains UI source of truth; SQLite mirrors register/post events.

use crate::late_compensation::InvoiceCompensationClaim;
use crate::late_interest::{InvoiceInterestClaim, ReferenceRateSource};
use crate::reminders::InvoiceReminder;
use crate::{InvoiceError, InvoiceId};
use klarbog_store_sqlite::{
    open_company, CompensationClaimRecord, CompensationPostingRecord, InterestClaimRecord,
    InterestPostingRecord, ReminderClaimRecord, ReminderPostingRecord,
};
use std::future::Future;
use std::path::Path;

const ACTOR: &str = "system";

fn block_on_store<T, F>(fut: F) -> Result<T, InvoiceError>
where
    F: Future<Output = Result<T, klarbog_store_sqlite::StoreError>> + Send,
    T: Send,
{
    // Claim register/post is sync (invoices.json), but sqlx is async. Never
    // `block_in_place` / nest `block_on` on the caller's runtime — UI tests
    // use current-thread tokio. Always dual-write on a dedicated thread.
    std::thread::scope(|s| {
        s.spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime for claim dual-write")
                .block_on(fut)
                .map_err(InvoiceError::from)
        })
        .join()
        .expect("claim dual-write thread")
    })
}

fn rate_source_label(source: ReferenceRateSource) -> &'static str {
    match source {
        ReferenceRateSource::StatutoryTable => "statutory-table",
        ReferenceRateSource::ManualOverride => "manual-override",
    }
}

pub fn dual_write_reminder(
    company: &Path,
    invoice_id: &InvoiceId,
    reminder: &InvoiceReminder,
) -> Result<bool, InvoiceError> {
    let record = ReminderClaimRecord {
        invoice_id: invoice_id.to_string(),
        reminder_date: reminder.reminder_date.clone(),
        fee_amount_minor: reminder.fee_amount_minor,
        currency: "DKK".into(),
        note: reminder.note.clone(),
    };
    let company = company.to_path_buf();
    block_on_store(async move {
        let store = open_company(&company).await?;
        store.record_reminder_claim(&record, ACTOR).await
    })
}

pub fn dual_write_interest(
    company: &Path,
    invoice_id: &InvoiceId,
    claim: &InvoiceInterestClaim,
) -> Result<bool, InvoiceError> {
    let record = InterestClaimRecord {
        invoice_id: invoice_id.to_string(),
        claim_date: claim.claim_date.clone(),
        reference_rate_bps: claim.reference_rate_bps,
        annual_interest_rate_bps: claim.annual_interest_rate_bps,
        reference_rate_source: rate_source_label(claim.reference_rate_source).into(),
        claimable_days: i64::from(claim.claimable_days),
        principal_open_minor: claim.principal_open_minor,
        amount_minor: claim.amount_minor,
        note: claim.note.clone(),
    };
    let company = company.to_path_buf();
    block_on_store(async move {
        let store = open_company(&company).await?;
        store.record_interest_claim(&record, ACTOR).await
    })
}

pub fn dual_write_compensation(
    company: &Path,
    invoice_id: &InvoiceId,
    claim: &InvoiceCompensationClaim,
) -> Result<bool, InvoiceError> {
    let record = CompensationClaimRecord {
        invoice_id: invoice_id.to_string(),
        claim_date: claim.claim_date.clone(),
        amount_minor: claim.amount_minor,
        note: claim.note.clone(),
    };
    let company = company.to_path_buf();
    block_on_store(async move {
        let store = open_company(&company).await?;
        store.record_compensation_claim(&record, ACTOR).await
    })
}

pub fn dual_write_reminder_posting(
    company: &Path,
    invoice_id: &InvoiceId,
    reminder_date: &str,
    journal_entry_id: &str,
) -> Result<(), InvoiceError> {
    let record = ReminderPostingRecord {
        invoice_id: invoice_id.to_string(),
        reminder_date: reminder_date.to_string(),
        journal_entry_id: journal_entry_id.to_string(),
    };
    let company = company.to_path_buf();
    block_on_store(async move {
        let store = open_company(&company).await?;
        store.record_reminder_posting(&record, ACTOR).await
    })
}

pub fn dual_write_interest_posting(
    company: &Path,
    invoice_id: &InvoiceId,
    claim_date: &str,
    reference_rate_bps: i64,
    journal_entry_id: &str,
) -> Result<(), InvoiceError> {
    let record = InterestPostingRecord {
        invoice_id: invoice_id.to_string(),
        claim_date: claim_date.to_string(),
        reference_rate_bps,
        journal_entry_id: journal_entry_id.to_string(),
    };
    let company = company.to_path_buf();
    block_on_store(async move {
        let store = open_company(&company).await?;
        store.record_interest_posting(&record, ACTOR).await
    })
}

pub fn dual_write_compensation_posting(
    company: &Path,
    invoice_id: &InvoiceId,
    claim_date: &str,
    journal_entry_id: &str,
) -> Result<(), InvoiceError> {
    let record = CompensationPostingRecord {
        invoice_id: invoice_id.to_string(),
        claim_date: claim_date.to_string(),
        journal_entry_id: journal_entry_id.to_string(),
    };
    let company = company.to_path_buf();
    block_on_store(async move {
        let store = open_company(&company).await?;
        store.record_compensation_posting(&record, ACTOR).await
    })
}
