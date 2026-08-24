//! Per-company `invoices.json` persistence (slice 6).

use crate::status::InvoiceStatus;
use crate::{Invoice, InvoiceError, InvoiceId, InvoiceKind, InvoiceLine, InvoiceVat};
use klarbog_plugin_crm::{get_party, PartyKind};
use klarbog_plugin_rules_dk::{split_vat25_inclusive, split_vat_from_net, DK_VAT_STANDARD_BPS};
use klarbog_types::{Currency, PartyId};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const INVOICES_FILENAME: &str = "invoices.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct InvoicesFile {
    pub(crate) invoices: Vec<Invoice>,
}

fn invoices_path(company: &Path) -> PathBuf {
    company.join(INVOICES_FILENAME)
}

pub(crate) fn load(company: &Path) -> Result<InvoicesFile, InvoiceError> {
    let path = invoices_path(company);
    if !path.exists() {
        return Ok(InvoicesFile::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub(crate) fn save(company: &Path, file: &InvoicesFile) -> Result<(), InvoiceError> {
    let json = serde_json::to_string_pretty(file)?;
    fs::write(invoices_path(company), json)?;
    Ok(())
}

pub fn list_invoices(company: &Path) -> Result<Vec<Invoice>, InvoiceError> {
    Ok(load(company)?.invoices)
}

pub fn get_invoice(company: &Path, id: &InvoiceId) -> Result<Option<Invoice>, InvoiceError> {
    Ok(load(company)?
        .invoices
        .into_iter()
        .find(|inv| inv.id == *id))
}

/// ADR-020: private parties are invoiced gross-inclusive, business parties
/// net-exclusive. Frozen on the invoice at creation.
fn vat_for(kind: PartyKind, line_total_minor: i64) -> Result<InvoiceVat, InvoiceError> {
    let split = match kind {
        PartyKind::Private => split_vat25_inclusive(line_total_minor)?,
        PartyKind::Business => split_vat_from_net(line_total_minor, DK_VAT_STANDARD_BPS)?,
    };
    Ok(InvoiceVat {
        net_minor: split.net_minor,
        vat_minor: split.vat_minor,
        gross_minor: split.gross_minor,
        rate_bps: split.rate_bps,
    })
}

pub fn create_draft(
    company: &Path,
    party_id: PartyId,
    kind: InvoiceKind,
    lines: Vec<InvoiceLine>,
    due_date: Option<String>,
) -> Result<Invoice, InvoiceError> {
    if lines.is_empty() {
        return Err(InvoiceError::NoLines);
    }
    let party = get_party(company, &party_id)?
        .ok_or_else(|| InvoiceError::PartyNotFound(party_id.to_string()))?;
    let mut invoice = Invoice {
        id: InvoiceId::generate(),
        party_id,
        kind,
        lines,
        status: InvoiceStatus::Draft,
        payments: Vec::new(),
        credits: Vec::new(),
        vat: None,
        credit_note_no: None,
        issue_date: None,
        due_date: None,
        invoice_no: None,
        issued_document_id: None,
        issued_sha256: None,
    };
    invoice.validate_lines()?;
    invoice.vat = Some(vat_for(party.kind, invoice.total_minor()?)?);
    if let Some(ref raw) = due_date {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            crate::due_date::parse_iso_date(trimmed)?;
            invoice.due_date = Some(trimmed.to_string());
        }
    }
    let mut file = load(company)?;
    file.invoices.push(invoice.clone());
    save(company, &file)?;
    Ok(invoice)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLine {
    pub description: String,
    pub amount_minor: i64,
    pub currency: String,
}

impl NewLine {
    pub fn into_line(self) -> Result<InvoiceLine, InvoiceError> {
        if self.description.trim().is_empty() {
            return Err(InvoiceError::EmptyDescription);
        }
        if self.amount_minor <= 0 {
            return Err(InvoiceError::NonPositiveAmount);
        }
        let currency = Currency::new(&self.currency).map_err(InvoiceError::Money)?;
        Ok(InvoiceLine {
            description: self.description,
            amount_minor: self.amount_minor,
            currency,
        })
    }
}

pub fn create_draft_from_new(
    company: &Path,
    party_id: PartyId,
    kind: InvoiceKind,
    raw_lines: Vec<NewLine>,
) -> Result<Invoice, InvoiceError> {
    create_draft_from_new_with_due(company, party_id, kind, raw_lines, None)
}

pub fn create_draft_from_new_with_due(
    company: &Path,
    party_id: PartyId,
    kind: InvoiceKind,
    raw_lines: Vec<NewLine>,
    due_date: Option<String>,
) -> Result<Invoice, InvoiceError> {
    let lines: Result<Vec<_>, _> = raw_lines.into_iter().map(NewLine::into_line).collect();
    create_draft(company, party_id, kind, lines?, due_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_crm::PartyKind;
    use tempfile::tempdir;

    #[test]
    fn persists_and_lists() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Buyer ApS".into(), PartyKind::Business).unwrap();
        let line = InvoiceLine {
            description: "Consulting".into(),
            amount_minor: 10_000,
            currency: Currency::new("DKK").unwrap(),
        };
        let inv = create_draft(&co, party.id.clone(), InvoiceKind::Sale, vec![line], None).unwrap();
        assert!(co.join(INVOICES_FILENAME).exists());
        assert_eq!(inv.status, InvoiceStatus::Draft);
        // Business → excl. VAT: net 10000, vat 2500, gross 12500.
        let vat = inv.vat.unwrap();
        assert_eq!(
            (vat.net_minor, vat.vat_minor, vat.gross_minor),
            (10_000, 2_500, 12_500)
        );
        assert_eq!(list_invoices(&co).unwrap().len(), 1);
        assert!(get_invoice(&co, &inv.id).unwrap().is_some());
    }

    #[test]
    fn private_party_amount_is_gross_inclusive() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(&co, None, "Privat Kunde".into(), PartyKind::Private).unwrap();
        let line = InvoiceLine {
            description: "Ydelse".into(),
            amount_minor: 12_500,
            currency: Currency::new("DKK").unwrap(),
        };
        let inv = create_draft(&co, party.id, InvoiceKind::Sale, vec![line], None).unwrap();
        // Private → incl. VAT: gross 12500 splits to net 10000 + vat 2500.
        let vat = inv.vat.unwrap();
        assert_eq!(
            (vat.net_minor, vat.vat_minor, vat.gross_minor),
            (10_000, 2_500, 12_500)
        );
        assert_eq!(inv.gross_minor().unwrap(), 12_500);
    }

    #[test]
    fn rejects_unknown_party() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let line = InvoiceLine {
            description: "x".into(),
            amount_minor: 100,
            currency: Currency::new("DKK").unwrap(),
        };
        let err = create_draft(
            &co,
            PartyId::new("party_missing"),
            InvoiceKind::Sale,
            vec![line],
            None,
        )
        .unwrap_err();
        assert!(matches!(err, InvoiceError::PartyNotFound(_)));
    }
}
