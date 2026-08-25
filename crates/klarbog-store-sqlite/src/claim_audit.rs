//! SQLite claim tables + audit_log (reminder / interest / compensation register).

use serde::{Deserialize, Serialize};

use crate::open::{CompanyStore, StoreError};

pub const REMINDER_REGISTER_AUDIT: &str = "invoice_reminder_register";
pub const INTEREST_REGISTER_AUDIT: &str = "invoice_interest_register";
pub const COMPENSATION_REGISTER_AUDIT: &str = "invoice_compensation_register";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReminderClaimRecord {
    pub invoice_id: String,
    pub reminder_date: String,
    pub fee_amount_minor: i64,
    pub currency: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterestClaimRecord {
    pub invoice_id: String,
    pub claim_date: String,
    pub reference_rate_bps: i64,
    pub annual_interest_rate_bps: i64,
    pub reference_rate_source: String,
    pub claimable_days: i64,
    pub principal_open_minor: i64,
    pub amount_minor: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationClaimRecord {
    pub invoice_id: String,
    pub claim_date: String,
    pub amount_minor: i64,
    pub note: Option<String>,
}

async fn insert_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_type: &str,
    entity_id: &str,
    message: &str,
    actor: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        r#"
        INSERT INTO audit_log (event_type, entity_type, entity_id, message, actor)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(event_type)
    .bind("invoice")
    .bind(entity_id)
    .bind(message)
    .bind(actor)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl CompanyStore {
    /// Insert reminder + audit. Idempotent on (invoice_id, reminder_date).
    pub async fn record_reminder_claim(
        &self,
        record: &ReminderClaimRecord,
        actor: &str,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO invoice_reminders (
                invoice_id, reminder_date, fee_amount_minor, currency, note
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&record.invoice_id)
        .bind(&record.reminder_date)
        .bind(record.fee_amount_minor)
        .bind(&record.currency)
        .bind(&record.note)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            tx.rollback().await?;
            return Ok(false);
        }
        let msg = format!(
            "Registered reminder fee {} øre on {} for invoice {}",
            record.fee_amount_minor, record.reminder_date, record.invoice_id
        );
        insert_audit(
            &mut tx,
            REMINDER_REGISTER_AUDIT,
            &record.invoice_id,
            &msg,
            actor,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Insert interest claim + audit. Idempotent on (invoice_id, claim_date, rate).
    pub async fn record_interest_claim(
        &self,
        record: &InterestClaimRecord,
        actor: &str,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO invoice_interest_claims (
                invoice_id, claim_date, reference_rate_bps, annual_interest_rate_bps,
                reference_rate_source, claimable_days, principal_open_minor,
                amount_minor, note
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&record.invoice_id)
        .bind(&record.claim_date)
        .bind(record.reference_rate_bps)
        .bind(record.annual_interest_rate_bps)
        .bind(&record.reference_rate_source)
        .bind(record.claimable_days)
        .bind(record.principal_open_minor)
        .bind(record.amount_minor)
        .bind(&record.note)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            tx.rollback().await?;
            return Ok(false);
        }
        let msg = format!(
            "Registered late interest {} øre on {} for invoice {} (ref {} bps)",
            record.amount_minor, record.claim_date, record.invoice_id, record.reference_rate_bps
        );
        insert_audit(
            &mut tx,
            INTEREST_REGISTER_AUDIT,
            &record.invoice_id,
            &msg,
            actor,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Insert compensation claim + audit. Idempotent on invoice_id UNIQUE.
    pub async fn record_compensation_claim(
        &self,
        record: &CompensationClaimRecord,
        actor: &str,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO invoice_compensation_claims (
                invoice_id, claim_date, amount_minor, note
            )
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&record.invoice_id)
        .bind(&record.claim_date)
        .bind(record.amount_minor)
        .bind(&record.note)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            tx.rollback().await?;
            return Ok(false);
        }
        let msg = format!(
            "Registered compensation {} øre on {} for invoice {}",
            record.amount_minor, record.claim_date, record.invoice_id
        );
        insert_audit(
            &mut tx,
            COMPENSATION_REGISTER_AUDIT,
            &record.invoice_id,
            &msg,
            actor,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn count_reminder_claims(&self) -> Result<i64, StoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice_reminders")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    pub async fn count_interest_claims(&self) -> Result<i64, StoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice_interest_claims")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    pub async fn count_compensation_claims(&self) -> Result<i64, StoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice_compensation_claims")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_company;
    use tempfile::tempdir;

    #[tokio::test]
    async fn reminder_insert_then_duplicate_is_idempotent() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        assert_eq!(store.schema_version().await.unwrap(), 5);
        let rec = ReminderClaimRecord {
            invoice_id: "inv_1".into(),
            reminder_date: "2026-06-26".into(),
            fee_amount_minor: 10_000,
            currency: "DKK".into(),
            note: None,
        };
        assert!(store.record_reminder_claim(&rec, "system").await.unwrap());
        assert!(!store.record_reminder_claim(&rec, "system").await.unwrap());
        assert_eq!(store.count_reminder_claims().await.unwrap(), 1);
        let audits = store
            .list_audit_by_event(REMINDER_REGISTER_AUDIT)
            .await
            .unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].entity_id.as_deref(), Some("inv_1"));
    }

    #[tokio::test]
    async fn interest_and_compensation_idempotent_uniques() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        let interest = InterestClaimRecord {
            invoice_id: "inv_2".into(),
            claim_date: "2026-07-01".into(),
            reference_rate_bps: 175,
            annual_interest_rate_bps: 975,
            reference_rate_source: "statutory-table".into(),
            claimable_days: 16,
            principal_open_minor: 125_000,
            amount_minor: 534,
            note: None,
        };
        assert!(store
            .record_interest_claim(&interest, "system")
            .await
            .unwrap());
        assert!(!store
            .record_interest_claim(&interest, "system")
            .await
            .unwrap());
        assert_eq!(store.count_interest_claims().await.unwrap(), 1);

        let comp = CompensationClaimRecord {
            invoice_id: "inv_2".into(),
            claim_date: "2026-07-01".into(),
            amount_minor: 31_000,
            note: None,
        };
        assert!(store
            .record_compensation_claim(&comp, "system")
            .await
            .unwrap());
        assert!(!store
            .record_compensation_claim(&comp, "system")
            .await
            .unwrap());
        assert_eq!(store.count_compensation_claims().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(INTEREST_REGISTER_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .list_audit_by_event(COMPENSATION_REGISTER_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
