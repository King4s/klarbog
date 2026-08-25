//! SQLite email_send_log + audit_log (DK-EMAIL-DELIVERY-001).

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::open::{CompanyStore, StoreError};

pub const EMAIL_AUDIT_EVENT: &str = "invoice_email_send";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailSendRecord {
    pub invoice_id: String,
    pub invoice_no: Option<String>,
    pub kind: String,
    pub recipient: String,
    pub sender: String,
    pub subject: String,
    pub message_id: String,
    pub body_sha256: String,
    pub smtp_host: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLogRow {
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub message: String,
    pub actor: String,
}

impl CompanyStore {
    pub async fn find_email_send_by_message_id(
        &self,
        message_id: &str,
    ) -> Result<Option<EmailSendRecord>, StoreError> {
        let row = sqlx::query(
            r#"
            SELECT invoice_id, invoice_no, kind, recipient, sender, subject,
                   message_id, body_sha256, smtp_host
            FROM email_send_log
            WHERE message_id = ?1
            LIMIT 1
            "#,
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| EmailSendRecord {
            invoice_id: r.get("invoice_id"),
            invoice_no: r.get("invoice_no"),
            kind: r.get("kind"),
            recipient: r.get("recipient"),
            sender: r.get("sender"),
            subject: r.get("subject"),
            message_id: r.get("message_id"),
            body_sha256: r.get("body_sha256"),
            smtp_host: r.get("smtp_host"),
        }))
    }

    /// Insert send log + audit. Idempotent on `message_id` UNIQUE — returns
    /// `false` when the row already existed (no second audit row).
    pub async fn record_email_send(
        &self,
        record: &EmailSendRecord,
        actor: &str,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO email_send_log (
                invoice_id, invoice_no, kind, recipient, sender, subject,
                message_id, body_sha256, smtp_host
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&record.invoice_id)
        .bind(&record.invoice_no)
        .bind(&record.kind)
        .bind(&record.recipient)
        .bind(&record.sender)
        .bind(&record.subject)
        .bind(&record.message_id)
        .bind(&record.body_sha256)
        .bind(&record.smtp_host)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            tx.rollback().await?;
            return Ok(false);
        }
        let invoice_no = record.invoice_no.as_deref().unwrap_or("?");
        let audit_msg = format!(
            "Sent {} email for invoice {} to {} via {} (message-id {})",
            record.kind, invoice_no, record.recipient, record.smtp_host, record.message_id
        );
        sqlx::query(
            r#"
            INSERT INTO audit_log (event_type, entity_type, entity_id, message, actor)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(EMAIL_AUDIT_EVENT)
        .bind("invoice")
        .bind(&record.invoice_id)
        .bind(&audit_msg)
        .bind(actor)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn count_email_sends(&self) -> Result<i64, StoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_send_log")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    pub async fn list_audit_by_event(
        &self,
        event_type: &str,
    ) -> Result<Vec<AuditLogRow>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT event_type, entity_type, entity_id, message, actor
            FROM audit_log
            WHERE event_type = ?1
            ORDER BY id ASC
            "#,
        )
        .bind(event_type)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| AuditLogRow {
                event_type: r.get("event_type"),
                entity_type: r.get("entity_type"),
                entity_id: r.get("entity_id"),
                message: r.get("message"),
                actor: r.get("actor"),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_company;
    use tempfile::tempdir;

    fn sample(message_id: &str) -> EmailSendRecord {
        EmailSendRecord {
            invoice_id: "inv_1".into(),
            invoice_no: Some("2026-0001".into()),
            kind: "invoice".into(),
            recipient: "buyer@example.com".into(),
            sender: "info@example.com".into(),
            subject: "Faktura 2026-0001".into(),
            message_id: message_id.into(),
            body_sha256: "abc".into(),
            smtp_host: "localhost".into(),
        }
    }

    #[tokio::test]
    async fn insert_then_duplicate_message_id_is_idempotent() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        assert_eq!(store.schema_version().await.unwrap(), 4);
        let rec = sample("<msg-1@klarbog.local>");
        assert!(store.record_email_send(&rec, "system").await.unwrap());
        assert!(!store.record_email_send(&rec, "system").await.unwrap());
        assert_eq!(store.count_email_sends().await.unwrap(), 1);
        let audits = store.list_audit_by_event(EMAIL_AUDIT_EVENT).await.unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].entity_id.as_deref(), Some("inv_1"));
        assert!(audits[0].message.contains("buyer@example.com"));
        assert!(!audits[0].message.to_lowercase().contains("password"));
    }
}
