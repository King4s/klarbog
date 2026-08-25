//! SQLite company store: WAL, foreign_keys, sqlx migrations (ADR-003/004).

mod email_audit;
mod open;
mod persist;
mod query;
mod reconcile_query;

pub use email_audit::{AuditLogRow, EmailSendRecord, EMAIL_AUDIT_EVENT};
pub use open::{open_company, CompanyStore, StoreError};
pub use persist::{JournalDigestSummary, JournalRetentionRow};
pub use query::{AccountBalance, PartyBalance, PostedEntryView, PostedLegView};
pub use reconcile_query::BankMemoRef;
