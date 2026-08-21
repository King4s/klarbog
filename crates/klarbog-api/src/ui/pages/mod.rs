//! Askama page handlers (ADR-018).

mod bank;
mod bilag;
mod chart;
pub(super) mod common;
mod home;
mod invoices;
mod journal;
mod parties;

pub use bank::{bank_get, bank_post};
pub use bilag::{bilag_attach, bilag_get, bilag_post};
pub use chart::{chart_get, chart_post};
pub use home::{home, settings_get, settings_post};
pub use invoices::{invoices_get, invoices_post};
pub use journal::{journal_get, journal_post};
pub use parties::{parties_get, parties_post};
