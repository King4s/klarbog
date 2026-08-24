//! Invoice lifecycle status persisted in `invoices.json`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    #[default]
    Draft,
    Sent,
    PartPaid,
    Paid,
    Void,
}

impl InvoiceStatus {
    pub fn allows_patch_from(self) -> bool {
        !matches!(self, InvoiceStatus::Void)
    }

    pub fn allows_mark_paid(self) -> bool {
        matches!(self, InvoiceStatus::Sent | InvoiceStatus::PartPaid)
    }

    /// Same gate as mark-paid: partial payments only from sent or already part_paid.
    pub fn allows_mark_part_paid(self) -> bool {
        matches!(self, InvoiceStatus::Sent | InvoiceStatus::PartPaid)
    }

    /// Credit notes (full or partial) only from sent, unpaid invoices.
    pub fn allows_credit(self) -> bool {
        matches!(self, InvoiceStatus::Sent)
    }

    /// Morarente only on sent/part_paid sale invoices with collectible balance.
    pub fn allows_late_interest(self) -> bool {
        matches!(self, InvoiceStatus::Sent | InvoiceStatus::PartPaid)
    }

    /// Rykkergebyr — same gate as morarente (sent/part_paid, not draft/paid/void).
    pub fn allows_reminder(self) -> bool {
        self.allows_late_interest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip() {
        for raw in ["draft", "sent", "part_paid", "paid", "void"] {
            let status: InvoiceStatus = serde_json::from_value(serde_json::json!(raw)).unwrap();
            let out = serde_json::to_value(status).unwrap();
            assert_eq!(out.as_str().unwrap(), raw);
        }
    }

    #[test]
    fn mark_paid_requires_sent_or_part_paid() {
        assert!(InvoiceStatus::Sent.allows_mark_paid());
        assert!(InvoiceStatus::PartPaid.allows_mark_paid());
        assert!(!InvoiceStatus::Draft.allows_mark_paid());
        assert!(!InvoiceStatus::Paid.allows_mark_paid());
        assert!(!InvoiceStatus::Void.allows_mark_paid());
    }

    #[test]
    fn mark_part_paid_same_gate() {
        assert!(InvoiceStatus::Sent.allows_mark_part_paid());
        assert!(InvoiceStatus::PartPaid.allows_mark_part_paid());
        assert!(!InvoiceStatus::Draft.allows_mark_part_paid());
        assert!(!InvoiceStatus::Paid.allows_mark_part_paid());
    }
}
