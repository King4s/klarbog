//! SQLite company store: WAL, foreign_keys, sqlx migrations (ADR-003/004).

mod claim_audit;
mod claim_postings;
mod email_audit;
mod open;
mod persist;
mod query;
mod reconcile_query;

pub use claim_audit::{
    CompensationClaimRecord, InterestClaimRecord, ReminderClaimRecord, COMPENSATION_REGISTER_AUDIT,
    INTEREST_REGISTER_AUDIT, REMINDER_REGISTER_AUDIT,
};
pub use claim_postings::{
    CompensationPostingRecord, InterestPostingRecord, ReminderPostingRecord,
    COMPENSATION_POST_AUDIT, INTEREST_POST_AUDIT, REMINDER_POST_AUDIT,
};
pub use email_audit::{AuditLogRow, EmailSendRecord, EMAIL_AUDIT_EVENT};
pub use open::{open_company, CompanyStore, StoreError};
pub use persist::{JournalDigestSummary, JournalRetentionRow};
pub use query::{AccountBalance, PartyBalance, PostedEntryView, PostedLegView};
pub use reconcile_query::BankMemoRef;
