//! SQLite company store: WAL, foreign_keys, migrations (ADR-003/004).

mod open;

pub use open::{open_company, CompanyStore, StoreError};
