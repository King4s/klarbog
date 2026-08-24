//! GDPR subject export v1 — company-scoped metadata only (no binary blobs).

use crate::retention::{load_retention, RetentionError, RetentionPolicy};
use klarbog_plugin_crm::list_parties;
use klarbog_plugin_documents::{list_documents, list_exceptions};
use klarbog_plugin_invoice::{list_invoices, InvoiceKind, InvoiceStatus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const GDPR_EXPORT_FILENAME: &str = "gdpr_export.json";

/// Fixed note on every export: journal is immutable; use party erasure for CRM/docs.
pub const GDPR_EXPORT_NOTE: &str = "Company-scoped metadata only (no binary blobs). Confirmed journal entries are immutable — party_id on posted legs is retained after GDPR party erasure; anonymize CRM display_name and strip or delete document metadata via erase-party.";

#[derive(Debug, Error)]
pub enum GdprError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("crm: {0}")]
    Crm(#[from] klarbog_plugin_crm::CrmError),
    #[error("documents: {0}")]
    Documents(#[from] klarbog_plugin_documents::DocumentError),
    #[error("invoice: {0}")]
    Invoice(#[from] klarbog_plugin_invoice::InvoiceError),
    #[error("retention: {0}")]
    Retention(#[from] RetentionError),
    #[error("store: {0}")]
    Store(#[from] klarbog_store_sqlite::StoreError),
    #[error("party not found: {0}")]
    PartyNotFound(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprParty {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprInvoice {
    pub id: String,
    pub party_id: String,
    pub status: InvoiceStatus,
    pub kind: InvoiceKind,
    pub total_minor: i64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprDocument {
    pub id: String,
    pub kind: String,
    pub path_hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub party_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprException {
    pub id: String,
    pub code: String,
    pub open: bool,
    pub related_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprRetentionSummary {
    pub retain_days: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purge_closed_exceptions_after_days: Option<i64>,
}

impl From<&RetentionPolicy> for GdprRetentionSummary {
    fn from(p: &RetentionPolicy) -> Self {
        Self {
            retain_days: p.retain_days,
            purge_closed_exceptions_after_days: p.purge_closed_exceptions_after_days,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GdprExport {
    pub exported_unix_ms: i64,
    pub note: String,
    pub parties: Vec<GdprParty>,
    pub invoices: Vec<GdprInvoice>,
    pub documents: Vec<GdprDocument>,
    pub exceptions: Vec<GdprException>,
    pub retention: GdprRetentionSummary,
}

fn kind_label(kind: klarbog_plugin_documents::DocumentKind) -> String {
    match kind {
        klarbog_plugin_documents::DocumentKind::Receipt => "receipt".into(),
        klarbog_plugin_documents::DocumentKind::InvoiceScan => "invoice_scan".into(),
        klarbog_plugin_documents::DocumentKind::CreditNote => "credit_note".into(),
        klarbog_plugin_documents::DocumentKind::Other => "other".into(),
    }
}

pub fn build_gdpr_export(company: &Path) -> Result<GdprExport, GdprError> {
    let parties = list_parties(company)?
        .into_iter()
        .map(|p| GdprParty {
            id: p.id.to_string(),
            display_name: p.display_name,
        })
        .collect();
    let mut invoices = Vec::new();
    for inv in list_invoices(company)? {
        let currency = inv
            .lines
            .first()
            .map(|l| l.currency.as_str().to_string())
            .unwrap_or_default();
        invoices.push(GdprInvoice {
            id: inv.id.to_string(),
            party_id: inv.party_id.to_string(),
            status: inv.status,
            kind: inv.kind,
            total_minor: inv.total_minor()?,
            currency,
        });
    }
    let documents = list_documents(company)?
        .into_iter()
        .map(|d| GdprDocument {
            id: d.id.to_string(),
            kind: kind_label(d.kind),
            path_hint: d.path_hint,
            party_id: d.party_id.map(|id| id.to_string()),
            invoice_id: d.invoice_id.map(|id| id.to_string()),
        })
        .collect();
    let exceptions = list_exceptions(company, false)?
        .into_iter()
        .map(|e| GdprException {
            id: e.id.to_string(),
            code: e.code,
            open: e.open,
            related_ids: e.related_ids,
        })
        .collect();
    let retention = GdprRetentionSummary::from(&load_retention(company)?);
    Ok(GdprExport {
        exported_unix_ms: chrono::Utc::now().timestamp_millis(),
        note: GDPR_EXPORT_NOTE.into(),
        parties,
        invoices,
        documents,
        exceptions,
        retention,
    })
}

pub fn write_gdpr_export(company: &Path) -> Result<GdprExport, GdprError> {
    let export = build_gdpr_export(company)?;
    let json = serde_json::to_string_pretty(&export)?;
    fs::write(company.join(GDPR_EXPORT_FILENAME), json)?;
    Ok(export)
}

pub fn gdpr_export_path(company: &Path) -> PathBuf {
    company.join(GDPR_EXPORT_FILENAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_documents::{
        attach_document, raise_exception, DocumentKind, ExceptionSeverity,
    };
    use klarbog_plugin_invoice::{create_draft, InvoiceKind, InvoiceLine, InvoiceStatus};
    use klarbog_types::Currency;
    use tempfile::tempdir;

    #[tokio::test]
    async fn v1_exports_metadata_only() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Test Person".into(),
            klarbog_plugin_crm::PartyKind::Private,
        )
        .unwrap();
        let inv = create_draft(
            &co,
            party.id.clone(),
            InvoiceKind::Sale,
            vec![InvoiceLine {
                description: "Widget".into(),
                amount_minor: 2500,
                currency: Currency::new("DKK").unwrap(),
            }],
            None,
        )
        .unwrap();
        attach_document(
            &co,
            DocumentKind::Receipt,
            "attachments/r.pdf".into(),
            Some(party.id.clone()),
            Some(inv.id.clone()),
            None,
            None,
        )
        .await
        .unwrap();
        raise_exception(
            &co,
            "missing_vat".into(),
            ExceptionSeverity::Warn,
            "check".into(),
            vec![party.id.to_string()],
        )
        .unwrap();
        let export = write_gdpr_export(&co).unwrap();
        assert_eq!(export.parties.len(), 1);
        assert_eq!(export.parties[0].display_name, "Test Person");
        assert_eq!(export.invoices.len(), 1);
        assert_eq!(export.invoices[0].total_minor, 2500);
        assert_eq!(export.invoices[0].currency, "DKK");
        assert_eq!(export.invoices[0].status, InvoiceStatus::Draft);
        assert_eq!(export.documents.len(), 1);
        assert_eq!(export.documents[0].path_hint, "attachments/r.pdf");
        assert_eq!(
            export.documents[0].party_id.as_deref(),
            Some(party.id.as_str())
        );
        assert_eq!(
            export.documents[0].invoice_id.as_deref(),
            Some(inv.id.as_str())
        );
        assert_eq!(export.exceptions.len(), 1);
        assert_eq!(export.exceptions[0].code, "missing_vat");
        assert!(export.exceptions[0].open);
        assert!(export.retention.retain_days > 0);
        assert_eq!(export.note, GDPR_EXPORT_NOTE);
        assert!(export.note.contains("immutable"));
        assert!(gdpr_export_path(&co).exists());
        let raw = fs::read_to_string(gdpr_export_path(&co)).unwrap();
        assert!(!raw.contains("content_base64"));
        assert!(!raw.contains("\\\"bytes\\\""));
    }
}
