//! Invoice plugin — drafts in `invoices.json` + journal suggestions with party_id.
//! No journal-write capability (ADR-004).

mod draft;
mod lifecycle;
mod status;
mod store;

pub use draft::{
    journal_suggestion, payment_journal_suggestion, payment_journal_suggestion_amount,
    InvoiceConfig,
};
pub use lifecycle::{mark_paid_preview, mark_part_paid_preview, patch_status};
pub use status::InvoiceStatus;
pub use store::{
    create_draft, create_draft_from_new, get_invoice, list_invoices, NewLine, INVOICES_FILENAME,
};

use klarbog_plugin::{Capability, Plugin};
use klarbog_types::{Currency, MoneyError, PartyId};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceKind {
    Sale,
    Purchase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceId(String);

impl InvoiceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn generate() -> Self {
        Self(format!("inv_{}", uuid::Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InvoiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub description: String,
    pub amount_minor: i64,
    pub currency: Currency,
}

/// One recorded payment against an invoice (company `invoices.json` ledger).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoicePayment {
    pub unix_ms: i64,
    pub amount_minor: i64,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invoice {
    pub id: InvoiceId,
    pub party_id: PartyId,
    pub kind: InvoiceKind,
    pub lines: Vec<InvoiceLine>,
    #[serde(default)]
    pub status: InvoiceStatus,
    /// Cumulative payment ledger; remaining = total − sum(amount_minor).
    #[serde(default)]
    pub payments: Vec<InvoicePayment>,
}

impl Invoice {
    pub fn validate_lines(&self) -> Result<(), InvoiceError> {
        if self.lines.is_empty() {
            return Err(InvoiceError::NoLines);
        }
        let currency = &self.lines[0].currency;
        for line in &self.lines {
            if line.description.trim().is_empty() {
                return Err(InvoiceError::EmptyDescription);
            }
            if line.amount_minor <= 0 {
                return Err(InvoiceError::NonPositiveAmount);
            }
            if &line.currency != currency {
                return Err(InvoiceError::MixedCurrency);
            }
        }
        Ok(())
    }

    pub fn total_minor(&self) -> Result<i64, InvoiceError> {
        self.validate_lines()?;
        self.lines.iter().try_fold(0i64, |acc, l| {
            acc.checked_add(l.amount_minor)
                .ok_or(InvoiceError::Overflow)
        })
    }

    pub fn paid_minor(&self) -> Result<i64, InvoiceError> {
        self.payments.iter().try_fold(0i64, |acc, p| {
            if p.amount_minor <= 0 {
                return Err(InvoiceError::NonPositiveAmount);
            }
            acc.checked_add(p.amount_minor)
                .ok_or(InvoiceError::Overflow)
        })
    }

    /// Open balance: `total_minor − paid_minor` (i64 only; fail-closed on overflow).
    pub fn remaining_minor(&self) -> Result<i64, InvoiceError> {
        let total = self.total_minor()?;
        let paid = self.paid_minor()?;
        total.checked_sub(paid).ok_or(InvoiceError::Overflow)
    }
}

#[derive(Debug, Error)]
pub enum InvoiceError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("party not found: {0}")]
    PartyNotFound(String),
    #[error("invoice not found: {0}")]
    NotFound(String),
    #[error("invoice has no lines")]
    NoLines,
    #[error("line description must not be empty")]
    EmptyDescription,
    #[error("line amount must be positive")]
    NonPositiveAmount,
    #[error("partial amount {amount_minor} must be > 0 and < remaining {remaining_minor}")]
    InvalidPartialAmount {
        amount_minor: i64,
        remaining_minor: i64,
    },
    #[error("payment {amount_minor} exceeds remaining {remaining_minor}")]
    Overpay {
        amount_minor: i64,
        remaining_minor: i64,
    },
    #[error("cannot mark paid: remaining balance is 0")]
    NothingRemaining,
    #[error("mixed currencies in one invoice")]
    MixedCurrency,
    #[error("overflow")]
    Overflow,
    #[error("invalid status transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: InvoiceStatus,
        to: InvoiceStatus,
    },
    #[error(transparent)]
    Money(#[from] MoneyError),
    #[error(transparent)]
    Journal(#[from] klarbog_journal::JournalError),
    #[error(transparent)]
    Crm(#[from] klarbog_plugin_crm::CrmError),
}

pub struct InvoicePlugin;

impl Default for InvoicePlugin {
    fn default() -> Self {
        Self
    }
}

impl InvoicePlugin {
    pub fn create(
        &self,
        company: &Path,
        party_id: PartyId,
        kind: InvoiceKind,
        lines: Vec<NewLine>,
    ) -> Result<Invoice, InvoiceError> {
        create_draft_from_new(company, party_id, kind, lines)
    }

    pub fn list(&self, company: &Path) -> Result<Vec<Invoice>, InvoiceError> {
        list_invoices(company)
    }

    pub fn get(&self, company: &Path, id: &InvoiceId) -> Result<Option<Invoice>, InvoiceError> {
        get_invoice(company, id)
    }
}

impl Plugin for InvoicePlugin {
    fn id(&self) -> &'static str {
        "invoice"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::Read, Capability::CrmWrite]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_plugin_crm::{upsert_party, PARTIES_FILENAME};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn invoice_has_no_journal_write() {
        let p = InvoicePlugin;
        assert!(!p.has_journal_write());
    }

    #[test]
    fn draft_roundtrip_with_journal_suggestion() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Nordic Buyer".into()).unwrap();
        assert!(co.join(PARTIES_FILENAME).exists());
        let plugin = InvoicePlugin;
        let invoice = plugin
            .create(
                &co,
                party.id,
                InvoiceKind::Sale,
                vec![NewLine {
                    description: "Support".into(),
                    amount_minor: 5000,
                    currency: "DKK".into(),
                }],
            )
            .unwrap();
        let entry = journal_suggestion(
            &invoice,
            &klarbog_types::Actor::user("t"),
            &InvoiceConfig::default(),
        )
        .unwrap();
        assert!(entry
            .legs
            .iter()
            .all(|l| l.party_id.as_ref() == Some(&invoice.party_id)));
        assert_eq!(plugin.list(&co).unwrap().len(), 1);
        assert_eq!(invoice.status, InvoiceStatus::Draft);
    }

    #[test]
    fn lifecycle_patch_and_mark_paid() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Buyer".into()).unwrap();
        let plugin = InvoicePlugin;
        let invoice = plugin
            .create(
                &co,
                party.id,
                InvoiceKind::Sale,
                vec![NewLine {
                    description: "Item".into(),
                    amount_minor: 3000,
                    currency: "DKK".into(),
                }],
            )
            .unwrap();
        let sent = patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
        assert_eq!(sent.status, InvoiceStatus::Sent);
        let actor = klarbog_types::Actor::user("t");
        let (paid, entry) =
            mark_paid_preview(&co, &invoice.id, &actor, &InvoiceConfig::default()).unwrap();
        assert_eq!(paid.status, InvoiceStatus::Paid);
        assert!(entry
            .legs
            .iter()
            .all(|l| l.party_id.as_ref() == Some(&invoice.party_id)));
    }

    #[test]
    fn lifecycle_mark_part_paid() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Buyer".into()).unwrap();
        let plugin = InvoicePlugin;
        let invoice = plugin
            .create(
                &co,
                party.id.clone(),
                InvoiceKind::Sale,
                vec![NewLine {
                    description: "Item".into(),
                    amount_minor: 8000,
                    currency: "DKK".into(),
                }],
            )
            .unwrap();
        patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
        let actor = klarbog_types::Actor::user("t");
        let (part, entry) =
            mark_part_paid_preview(&co, &invoice.id, 2500, &actor, &InvoiceConfig::default())
                .unwrap();
        assert_eq!(part.status, InvoiceStatus::PartPaid);
        assert_eq!(part.payments.len(), 1);
        assert_eq!(entry.legs[0].amount.minor(), 2500);
        assert_eq!(part.remaining_minor().unwrap(), 5500);
        assert!(entry
            .legs
            .iter()
            .all(|l| l.party_id.as_ref() == Some(&party.id)));
    }
}
