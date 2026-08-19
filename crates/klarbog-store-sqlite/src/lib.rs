//! SQLite company store: WAL, foreign_keys, sqlx migrations (ADR-003/004).

mod open;
mod persist;

pub use open::{open_company, CompanyStore, StoreError};
