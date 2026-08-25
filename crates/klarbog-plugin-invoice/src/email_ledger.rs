//! Dual-write invoice email sends to company ledger SQLite.

use crate::email::{EmailKind, EmailSendLogRow};
use crate::InvoiceError;
use klarbog_store_sqlite::{open_company, EmailSendRecord};
use std::path::Path;

const ACTOR: &str = "system";

fn to_record(row: &EmailSendLogRow) -> EmailSendRecord {
    EmailSendRecord {
        invoice_id: row.invoice_id.clone(),
        invoice_no: Some(row.invoice_no.clone()),
        kind: match row.kind {
            EmailKind::Invoice => "invoice".into(),
            EmailKind::Reminder => "reminder".into(),
        },
        recipient: row.recipient.clone(),
        sender: row.sender.clone(),
        subject: row.subject.clone(),
        message_id: row.message_id.clone(),
        body_sha256: row.body_sha256.clone(),
        smtp_host: row.smtp_host.clone(),
    }
}

pub async fn find_sqlite_duplicate(
    company: &Path,
    message_id: &str,
) -> Result<Option<EmailSendLogRow>, InvoiceError> {
    let store = open_company(company).await?;
    Ok(store
        .find_email_send_by_message_id(message_id)
        .await?
        .map(|r| EmailSendLogRow {
            invoice_id: r.invoice_id,
            invoice_no: r.invoice_no.unwrap_or_default(),
            kind: if r.kind == "reminder" {
                EmailKind::Reminder
            } else {
                EmailKind::Invoice
            },
            recipient: r.recipient,
            sender: r.sender,
            subject: r.subject,
            message_id: r.message_id,
            body_sha256: r.body_sha256,
            smtp_host: r.smtp_host,
            unix_ms: 0,
        }))
}

/// Keep JSONL for backward compat; SQLite is the primary registry log.
pub async fn dual_write_send_log(
    company: &Path,
    row: &EmailSendLogRow,
) -> Result<bool, InvoiceError> {
    let store = open_company(company).await?;
    Ok(store.record_email_send(&to_record(row), ACTOR).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::email::{read_send_log, send_invoice_email, EmailKind, EMAIL_SEND_LOG};
    use crate::{create_draft_from_new, record_issue, InvoiceKind, NewLine};
    use klarbog_mail::SmtpConfig;
    use klarbog_plugin_crm::{upsert_party, PartyKind};
    use klarbog_storage::company_objects_root;
    use klarbog_store_sqlite::{open_company, EMAIL_AUDIT_EVENT};
    use std::fs;
    use tempfile::tempdir;

    fn issued_fixture(email: Option<&str>) -> (tempfile::TempDir, crate::InvoiceId, String) {
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

    #[tokio::test]
    async fn sqlite_row_and_audit_on_send() {
        let (_dir, inv_id, co_path) = issued_fixture(Some("buyer@example.com"));
        let co = Path::new(&co_path);
        let smtp = SmtpConfig::from_env();
        let out = send_invoice_email(co, &inv_id, EmailKind::Invoice, None, &smtp, true)
            .await
            .unwrap();
        assert!(!out.duplicate);
        assert!(co.join(EMAIL_SEND_LOG).exists());
        assert_eq!(read_send_log(co).unwrap().len(), 1);

        let store = open_company(co).await.unwrap();
        assert_eq!(store.count_email_sends().await.unwrap(), 1);
        let found = store
            .find_email_send_by_message_id(&out.message_id)
            .await
            .unwrap()
            .expect("sqlite row");
        assert_eq!(found.recipient, "buyer@example.com");
        let audits = store.list_audit_by_event(EMAIL_AUDIT_EVENT).await.unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].event_type, EMAIL_AUDIT_EVENT);
    }

    #[tokio::test]
    async fn duplicate_message_id_does_not_double_insert() {
        let (_dir, inv_id, co_path) = issued_fixture(Some("buyer@example.com"));
        let co = Path::new(&co_path);
        let smtp = SmtpConfig::from_env();
        let first = send_invoice_email(co, &inv_id, EmailKind::Invoice, None, &smtp, true)
            .await
            .unwrap();
        let second = send_invoice_email(co, &inv_id, EmailKind::Invoice, None, &smtp, true)
            .await
            .unwrap();
        assert!(second.duplicate);
        assert_eq!(first.message_id, second.message_id);
        let store = open_company(co).await.unwrap();
        assert_eq!(store.count_email_sends().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(EMAIL_AUDIT_EVENT)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(read_send_log(co).unwrap().len(), 1);
    }
}
