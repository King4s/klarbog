//! Issued-invoice metadata (DK-INVOICE-DUE-DATE-001).

use crate::due_date::{add_days, format_iso_date, parse_iso_date};
use crate::status::InvoiceStatus;
use crate::{Invoice, InvoiceError, InvoiceId};
use std::path::Path;

/// Bogfør/send: sæt udstedelsesdato, udfyld forfald hvis mangler, flip til sent.
pub fn record_issue(
    company: &Path,
    id: &InvoiceId,
    issue_date: String,
    payment_terms_days: u32,
) -> Result<Invoice, InvoiceError> {
    parse_iso_date(&issue_date)?;
    let mut file = crate::store::load(company)?;
    let invoice = file
        .invoices
        .iter_mut()
        .find(|inv| inv.id == *id)
        .ok_or_else(|| InvoiceError::NotFound(id.to_string()))?;
    if !invoice.status.allows_patch_from() {
        return Err(InvoiceError::InvalidTransition {
            from: invoice.status,
            to: InvoiceStatus::Sent,
        });
    }
    invoice.issue_date = Some(issue_date.clone());
    if invoice
        .due_date
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        let issue = parse_iso_date(&issue_date)?;
        invoice.due_date = Some(format_iso_date(add_days(
            issue,
            i64::from(payment_terms_days),
        )));
    } else if let Some(ref due) = invoice.due_date {
        let due_naive = parse_iso_date(due)?;
        let issue_naive = parse_iso_date(&issue_date)?;
        if due_naive < issue_naive {
            return Err(InvoiceError::DueBeforeIssue {
                due_date: due.clone(),
                issue_date,
            });
        }
    }
    invoice.status = InvoiceStatus::Sent;
    let updated = invoice.clone();
    crate::store::save(company, &file)?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{create_draft_from_new, NewLine};
    use crate::InvoiceKind;
    use klarbog_plugin_crm::upsert_party;
    use tempfile::tempdir;

    #[test]
    fn record_issue_sets_dates() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Buyer".into(),
            klarbog_plugin_crm::PartyKind::Private,
        )
        .unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Work".into(),
                amount_minor: 10_000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        let issued = record_issue(&co, &inv.id, "2026-05-16".into(), 30).unwrap();
        assert_eq!(issued.status, InvoiceStatus::Sent);
        assert_eq!(issued.issue_date.as_deref(), Some("2026-05-16"));
        assert_eq!(issued.due_date.as_deref(), Some("2026-06-15"));
        let assessment = issued
            .due_assessment(chrono::NaiveDate::from_ymd_opt(2026, 6, 20).unwrap())
            .unwrap();
        assert!(assessment.is_overdue);
        assert_eq!(assessment.overdue_days, 5);
    }
}
