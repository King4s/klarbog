//! Append-only claim posting links (reminder / interest / compensation).

use crate::open::{CompanyStore, StoreError};

pub const REMINDER_POST_AUDIT: &str = "invoice_reminder_post";
pub const INTEREST_POST_AUDIT: &str = "invoice_interest_post";
pub const COMPENSATION_POST_AUDIT: &str = "invoice_compensation_post";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderPostingRecord {
    pub invoice_id: String,
    pub reminder_date: String,
    pub journal_entry_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterestPostingRecord {
    pub invoice_id: String,
    pub claim_date: String,
    pub reference_rate_bps: i64,
    pub journal_entry_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationPostingRecord {
    pub invoice_id: String,
    pub claim_date: String,
    pub journal_entry_id: String,
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
    /// Insert reminder posting + audit. Fail-closed on duplicate claim identity.
    pub async fn record_reminder_posting(
        &self,
        record: &ReminderPostingRecord,
        actor: &str,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO invoice_reminder_postings (
                invoice_id, reminder_date, journal_entry_id
            )
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind(&record.invoice_id)
        .bind(&record.reminder_date)
        .bind(&record.journal_entry_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            tx.rollback().await?;
            return Err(StoreError::DuplicateClaimPosting);
        }
        let msg = format!(
            "Posted reminder fee on {} for invoice {} → journal {}",
            record.reminder_date, record.invoice_id, record.journal_entry_id
        );
        insert_audit(
            &mut tx,
            REMINDER_POST_AUDIT,
            &record.invoice_id,
            &msg,
            actor,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Insert interest posting + audit. Fail-closed on duplicate claim identity.
    pub async fn record_interest_posting(
        &self,
        record: &InterestPostingRecord,
        actor: &str,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO invoice_interest_postings (
                invoice_id, claim_date, reference_rate_bps, journal_entry_id
            )
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&record.invoice_id)
        .bind(&record.claim_date)
        .bind(record.reference_rate_bps)
        .bind(&record.journal_entry_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            tx.rollback().await?;
            return Err(StoreError::DuplicateClaimPosting);
        }
        let msg = format!(
            "Posted late interest on {}@{} bps for invoice {} → journal {}",
            record.claim_date,
            record.reference_rate_bps,
            record.invoice_id,
            record.journal_entry_id
        );
        insert_audit(
            &mut tx,
            INTEREST_POST_AUDIT,
            &record.invoice_id,
            &msg,
            actor,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Insert compensation posting + audit. Fail-closed on duplicate claim identity.
    pub async fn record_compensation_posting(
        &self,
        record: &CompensationPostingRecord,
        actor: &str,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO invoice_compensation_postings (
                invoice_id, claim_date, journal_entry_id
            )
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind(&record.invoice_id)
        .bind(&record.claim_date)
        .bind(&record.journal_entry_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            tx.rollback().await?;
            return Err(StoreError::DuplicateClaimPosting);
        }
        let msg = format!(
            "Posted compensation on {} for invoice {} → journal {}",
            record.claim_date, record.invoice_id, record.journal_entry_id
        );
        insert_audit(
            &mut tx,
            COMPENSATION_POST_AUDIT,
            &record.invoice_id,
            &msg,
            actor,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn count_reminder_postings(&self) -> Result<i64, StoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice_reminder_postings")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    pub async fn count_interest_postings(&self) -> Result<i64, StoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice_interest_postings")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    pub async fn count_compensation_postings(&self) -> Result<i64, StoreError> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice_compensation_postings")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_audit::{CompensationClaimRecord, InterestClaimRecord, ReminderClaimRecord};
    use crate::open_company;
    use tempfile::tempdir;

    async fn seed_claims(store: &CompanyStore) {
        store
            .record_reminder_claim(
                &ReminderClaimRecord {
                    invoice_id: "inv_1".into(),
                    reminder_date: "2026-06-26".into(),
                    fee_amount_minor: 10_000,
                    currency: "DKK".into(),
                    note: None,
                },
                "system",
            )
            .await
            .unwrap();
        store
            .record_interest_claim(
                &InterestClaimRecord {
                    invoice_id: "inv_1".into(),
                    claim_date: "2026-07-01".into(),
                    reference_rate_bps: 175,
                    annual_interest_rate_bps: 975,
                    reference_rate_source: "statutory-table".into(),
                    claimable_days: 16,
                    principal_open_minor: 125_000,
                    amount_minor: 534,
                    note: None,
                },
                "system",
            )
            .await
            .unwrap();
        store
            .record_compensation_claim(
                &CompensationClaimRecord {
                    invoice_id: "inv_1".into(),
                    claim_date: "2026-07-01".into(),
                    amount_minor: 31_000,
                    note: None,
                },
                "system",
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reminder_posting_once_then_duplicate_rejected() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        assert_eq!(store.schema_version().await.unwrap(), 5);
        seed_claims(&store).await;
        let post = ReminderPostingRecord {
            invoice_id: "inv_1".into(),
            reminder_date: "2026-06-26".into(),
            journal_entry_id: "je_1".into(),
        };
        store
            .record_reminder_posting(&post, "system")
            .await
            .unwrap();
        let err = store
            .record_reminder_posting(
                &ReminderPostingRecord {
                    journal_entry_id: "je_2".into(),
                    ..post.clone()
                },
                "system",
            )
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::DuplicateClaimPosting));
        assert_eq!(store.count_reminder_postings().await.unwrap(), 1);
        assert_eq!(
            store
                .list_audit_by_event(REMINDER_POST_AUDIT)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn interest_and_compensation_posting_reject_duplicates() {
        let dir = tempdir().unwrap();
        let store = open_company(dir.path()).await.unwrap();
        seed_claims(&store).await;

        let interest = InterestPostingRecord {
            invoice_id: "inv_1".into(),
            claim_date: "2026-07-01".into(),
            reference_rate_bps: 175,
            journal_entry_id: "je_i1".into(),
        };
        store
            .record_interest_posting(&interest, "system")
            .await
            .unwrap();
        assert!(matches!(
            store
                .record_interest_posting(
                    &InterestPostingRecord {
                        journal_entry_id: "je_i2".into(),
                        ..interest.clone()
                    },
                    "system",
                )
                .await
                .unwrap_err(),
            StoreError::DuplicateClaimPosting
        ));
        assert_eq!(store.count_interest_postings().await.unwrap(), 1);

        let comp = CompensationPostingRecord {
            invoice_id: "inv_1".into(),
            claim_date: "2026-07-01".into(),
            journal_entry_id: "je_c1".into(),
        };
        store
            .record_compensation_posting(&comp, "system")
            .await
            .unwrap();
        assert!(matches!(
            store
                .record_compensation_posting(
                    &CompensationPostingRecord {
                        journal_entry_id: "je_c2".into(),
                        ..comp.clone()
                    },
                    "system",
                )
                .await
                .unwrap_err(),
            StoreError::DuplicateClaimPosting
        ));
        assert_eq!(store.count_compensation_postings().await.unwrap(), 1);
    }
}
