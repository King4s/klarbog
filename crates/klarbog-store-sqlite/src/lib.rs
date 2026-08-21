//! SQLite company store: WAL, foreign_keys, sqlx migrations (ADR-003/004).

mod open;
mod persist;
mod query;

pub use open::{open_company, CompanyStore, StoreError};
pub use persist::JournalDigestSummary;
pub use query::{AccountBalance, PartyBalance, PostedEntryView, PostedLegView};
