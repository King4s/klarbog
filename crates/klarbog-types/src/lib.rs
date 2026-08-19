//! Klarbog shared types. Money is always integer minor units (ADR-001).

mod actor;
mod money;
mod party;
mod result;

pub use actor::{Actor, ActorKind};
pub use money::{Currency, MinorAmount, Money, MoneyError};
pub use party::PartyId;
pub use result::{Envelope, KlarbogError};
