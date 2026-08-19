//! Mandatory double-entry journal (ADR-004). Unbalanced entries are rejected.

mod entry;
mod hash;

pub use entry::{Direction, JournalEntry, JournalError, Leg, PostedEntry};
pub use hash::entry_digest;
