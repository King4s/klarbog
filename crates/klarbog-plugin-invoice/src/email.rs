//! Invoice email delivery (DK-EMAIL-DELIVERY-001).

use crate::{get_invoice, InvoiceError, InvoiceId, InvoiceStatus};
use klarbog_mail::{email_dry_run_from_env, send_mail_with_attachment, SmtpConfig};
use klarbog_plugin_crm::get_party;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::Path;

pub const RULE_ID: &str = "DK-EMAIL-DELIVERY-001";
pub const EMAIL_SEND_LOG: &str = "email_send_log.jsonl";
const MESSAGE_ID_DOMAIN: &str = "klarbog.local";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailKind {
    Invoice,
    Reminder,
}

impl EmailKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Invoice => "invoice",
            Self::Reminder => "reminder",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailSendLogRow {
    pub invoice_id: String,
    pub invoice_no: String,
    pub kind: EmailKind,
    pub recipient: String,
    pub sender: String,
    pub subject: String,
    pub message_id: String,
    pub body_sha256: String,
    pub smtp_host: String,
    pub unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendInvoiceEmailOutcome {
    pub message_id: String,
    pub recipient: String,
    pub subject: String,
    pub duplicate: bool,
}

pub fn looks_like_email(value: &str) -> bool {
    let t = value.trim();
    !t.is_empty() && t.contains('@') && !t.starts_with('@') && !t.ends_with('@') && !t.contains(' ')
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn subject_for(kind: EmailKind, invoice_no: &str) -> String {
    match kind {
        EmailKind::Reminder => format!("Betalingspåmindelse for faktura {invoice_no}"),
        EmailKind::Invoice => format!("Faktura {invoice_no}"),
    }
}

fn body_text_for(kind: EmailKind, invoice_no: &str) -> String {
    match kind {
        EmailKind::Reminder => format!(
            "Hej\r\n\r\nVi kan se at faktura {invoice_no} endnu ikke er betalt. \
             Fakturaen er vedhæftet som PDF. Kontakt os hvis betalingen allerede er gennemført.\r\n\r\n\
             Venlig hilsen\r\nKlarbog"
        ),
        EmailKind::Invoice => format!(
            "Hej\r\n\r\nHermed faktura {invoice_no}, vedhæftet som PDF.\r\n\r\n\
             Venlig hilsen\r\nKlarbog"
        ),
    }
}

/// Deterministic message-id from content fingerprint (no timestamps).
pub fn deterministic_message_id(
    kind: EmailKind,
    invoice_no: &str,
    to: &str,
    from_email: &str,
    attachment_sha256: &str,
) -> String {
    let fingerprint = sha256_hex(
        format!(
            "{}|{}|{}|{}|{}",
            kind.as_str(),
            invoice_no,
            to.trim(),
            from_email.trim(),
            attachment_sha256
        )
        .as_bytes(),
    );
    format!("<{}@{}>", &fingerprint[..32], MESSAGE_ID_DOMAIN)
}

fn log_path(company: &Path) -> std::path::PathBuf {
    company.join(EMAIL_SEND_LOG)
}

pub fn read_send_log(company: &Path) -> Result<Vec<EmailSendLogRow>, InvoiceError> {
    let path = log_path(company);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(&path)?;
    let mut rows = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line)?);
    }
    Ok(rows)
}

fn append_send_log(company: &Path, row: &EmailSendLogRow) -> Result<(), InvoiceError> {
    let path = log_path(company);
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    serde_json::to_writer(&mut file, row)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn find_duplicate_message_id(
    company: &Path,
    message_id: &str,
) -> Result<Option<EmailSendLogRow>, InvoiceError> {
    Ok(read_send_log(company)?
        .into_iter()
        .find(|r| r.message_id == message_id))
}

fn issued_status_ok(status: InvoiceStatus) -> bool {
    matches!(
        status,
        InvoiceStatus::Sent | InvoiceStatus::PartPaid | InvoiceStatus::Paid
    )
}

/// Send an issued invoice (or reminder) by email with the issued PDF attached.
pub async fn send_invoice_email(
    company: &Path,
    invoice_id: &InvoiceId,
    kind: EmailKind,
    to_override: Option<&str>,
    smtp: &SmtpConfig,
    dry_run: bool,
) -> Result<SendInvoiceEmailOutcome, InvoiceError> {
    let invoice = get_invoice(company, invoice_id)?
        .ok_or_else(|| InvoiceError::NotFound(invoice_id.to_string()))?;

    if !issued_status_ok(invoice.status) {
        return Err(InvoiceError::NotSentForEmail);
    }

    let invoice_no = invoice
        .invoice_no
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .ok_or(InvoiceError::MissingIssuedDocument)?;

    let doc_id = invoice
        .issued_document_id
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .ok_or(InvoiceError::MissingIssuedDocument)?;
    let _issued_doc = doc_id;

    let party = get_party(company, &invoice.party_id)?
        .ok_or_else(|| InvoiceError::PartyNotFound(invoice.party_id.to_string()))?;

    let recipient = if let Some(explicit) = to_override.filter(|s| !s.trim().is_empty()) {
        explicit.trim().to_string()
    } else {
        party
            .email
            .clone()
            .filter(|e| !e.trim().is_empty())
            .ok_or_else(|| InvoiceError::MissingRecipientEmail(invoice_no.clone()))?
    };

    if !looks_like_email(&recipient) {
        return Err(InvoiceError::InvalidRecipientEmail(recipient));
    }

    let attachment_bytes = crate::email_pdf::resolve_invoice_pdf_bytes(
        company,
        &invoice,
        invoice_no,
        Some(party.display_name.as_str()),
    )?;
    let attachment_sha256 = sha256_hex(&attachment_bytes);

    let subject = subject_for(kind, invoice_no);
    let body = body_text_for(kind, invoice_no);
    let message_id = deterministic_message_id(
        kind,
        invoice_no,
        &recipient,
        &smtp.from_email,
        &attachment_sha256,
    );

    if let Some(existing) = find_duplicate_message_id(company, &message_id)? {
        return Ok(SendInvoiceEmailOutcome {
            message_id: existing.message_id,
            recipient: existing.recipient,
            subject: existing.subject,
            duplicate: true,
        });
    }
    if let Some(existing) = crate::email_ledger::find_sqlite_duplicate(company, &message_id).await?
    {
        return Ok(SendInvoiceEmailOutcome {
            message_id: existing.message_id,
            recipient: existing.recipient,
            subject: existing.subject,
            duplicate: true,
        });
    }

    let filename = format!("{invoice_no}.pdf");
    let effective_dry_run = dry_run || email_dry_run_from_env() || !smtp.is_configured();

    send_mail_with_attachment(
        smtp,
        &recipient,
        &subject,
        &body,
        &filename,
        "application/pdf",
        &attachment_bytes,
        effective_dry_run,
    )
    .await
    .map_err(InvoiceError::EmailSendFailed)?;

    let body_sha256 = sha256_hex(body.as_bytes());
    let row = EmailSendLogRow {
        invoice_id: invoice_id.to_string(),
        invoice_no: invoice_no.clone(),
        kind,
        recipient: recipient.clone(),
        sender: smtp.from_email.clone(),
        subject: subject.clone(),
        message_id: message_id.clone(),
        body_sha256,
        smtp_host: smtp.host.clone(),
        unix_ms: chrono::Utc::now().timestamp_millis(),
    };
    append_send_log(company, &row)?;
    crate::email_ledger::dual_write_send_log(company, &row).await?;

    Ok(SendInvoiceEmailOutcome {
        message_id,
        recipient,
        subject,
        duplicate: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_draft_from_new, record_issue, InvoiceKind, NewLine};
    use klarbog_plugin_crm::{upsert_party, PartyKind};
    use klarbog_storage::company_objects_root;
    use tempfile::tempdir;

    fn issued_invoice_fixture(email: Option<&str>) -> (tempfile::TempDir, InvoiceId, String) {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Buyer".into(),
            PartyKind::Private,
            None,
            email.map(|e| e.to_string()),
        )
        .unwrap();
        let inv = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Widget".into(),
                amount_minor: 12_500,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        let object_dir = company_objects_root(&co).join("invoices/issued");
        fs::create_dir_all(&object_dir).unwrap();
        fs::write(
            object_dir.join("2026-0001.json"),
            br#"{"type":"issued_invoice","invoiceNumber":"2026-0001"}"#,
        )
        .unwrap();
        record_issue(
            &co,
            &inv.id,
            "2026-05-16".into(),
            30,
            Some("2026-0001".into()),
            Some("doc_test_issued".into()),
            Some("deadbeef".into()),
        )
        .unwrap();
        (dir, inv.id, co.to_string_lossy().to_string())
    }

    #[test]
    fn message_id_is_deterministic() {
        let a = deterministic_message_id(
            EmailKind::Invoice,
            "2026-0001",
            "a@b.dk",
            "info+klarbog@pellucidsoftware.com",
            "abc123",
        );
        let b = deterministic_message_id(
            EmailKind::Invoice,
            "2026-0001",
            "a@b.dk",
            "info+klarbog@pellucidsoftware.com",
            "abc123",
        );
        assert_eq!(a, b);
        assert!(a.starts_with('<') && a.ends_with('>'));
    }

    #[tokio::test]
    async fn duplicate_send_is_idempotent() {
        let (_dir, inv_id, co_path) = issued_invoice_fixture(Some("buyer@example.com"));
        let co = Path::new(&co_path);
        let smtp = SmtpConfig::from_env();
        let first = send_invoice_email(co, &inv_id, EmailKind::Invoice, None, &smtp, true)
            .await
            .unwrap();
        assert!(!first.duplicate);
        let second = send_invoice_email(co, &inv_id, EmailKind::Invoice, None, &smtp, true)
            .await
            .unwrap();
        assert!(second.duplicate);
        assert_eq!(first.message_id, second.message_id);
        assert_eq!(read_send_log(co).unwrap().len(), 1);
        let body = body_text_for(EmailKind::Invoice, "2026-0001");
        assert!(body.contains("vedhæftet som PDF"));
    }

    #[tokio::test]
    async fn dry_run_attaches_generated_pdf_when_snapshot_missing() {
        let (_dir, inv_id, co_path) = issued_invoice_fixture(Some("buyer@example.com"));
        let co = Path::new(&co_path);
        // Fixture only wrote JSON — PDF must be generated on the fly.
        assert!(!company_objects_root(co)
            .join("invoices/issued/2026-0001.pdf")
            .exists());
        let smtp = SmtpConfig::from_env();
        let out = send_invoice_email(co, &inv_id, EmailKind::Invoice, None, &smtp, true)
            .await
            .unwrap();
        assert!(!out.duplicate);
        assert!(out.subject.contains("2026-0001"));
    }

    #[tokio::test]
    async fn missing_recipient_fails_closed() {
        let (_dir, inv_id, co_path) = issued_invoice_fixture(None);
        let co = Path::new(&co_path);
        let smtp = SmtpConfig::from_env();
        let err = send_invoice_email(co, &inv_id, EmailKind::Invoice, None, &smtp, true)
            .await
            .unwrap_err();
        assert!(matches!(err, InvoiceError::MissingRecipientEmail(_)));
        assert!(read_send_log(co).unwrap().is_empty());
    }
}
