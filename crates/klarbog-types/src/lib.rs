//! Klarbog shared types. Money is always integer minor units (ADR-001).

mod actor;
mod money;
mod party;
mod result;
mod retention_deadline;

pub use actor::{Actor, ActorKind};
pub use money::{Currency, MinorAmount, Money, MoneyError};
pub use party::PartyId;
pub use result::{Envelope, KlarbogError};
pub use retention_deadline::{
    effective_retain_until, parse_iso_date, retain_until_for_date, retain_until_for_iso_date,
    retain_until_iso, RetentionDeadlineError, RETENTION_RULE_ID,
};
