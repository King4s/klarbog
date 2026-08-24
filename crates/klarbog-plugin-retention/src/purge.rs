//! Retention purge: closed exceptions + optional orphan document metadata GC.

use crate::{load_retention, RetentionError};
use klarbog_plugin_documents::{find_orphan_documents, purge_closed_exceptions, remove_document};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

const MS_PER_DAY: i64 = 86_400_000;

#[derive(Debug, Error)]
pub enum PurgeError {
    #[error("retention: {0}")]
    Retention(#[from] RetentionError),
    #[error("documents: {0}")]
    Documents(#[from] klarbog_plugin_documents::DocumentError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeReport {
    pub dry_run: bool,
    pub purge_closed_exceptions_after_days: Option<i64>,
    pub exceptions_purged: Vec<String>,
    pub orphan_documents_gc: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PurgeOptions {
    pub confirm: bool,
    pub gc_orphan_documents: bool,
}

pub async fn run_retention_purge(
    company: &Path,
    options: PurgeOptions,
) -> Result<PurgeReport, PurgeError> {
    let policy = load_retention(company)?;
    let dry_run = !options.confirm;
    let mut exceptions_purged = Vec::new();
    let mut orphan_documents_gc = Vec::new();

    if let Some(days) = policy.purge_closed_exceptions_after_days {
        let cutoff_ms = chrono::Utc::now().timestamp_millis() - days * MS_PER_DAY;
        let removed = purge_closed_exceptions(company, cutoff_ms, dry_run)?;
        exceptions_purged = removed.into_iter().map(|id| id.to_string()).collect();
    }

    if options.gc_orphan_documents {
        let orphans = find_orphan_documents(company)?;
        if dry_run {
            orphan_documents_gc = orphans.into_iter().map(|id| id.to_string()).collect();
        } else {
            for id in orphans {
                let removed = remove_document(company, &id, false).await?;
                orphan_documents_gc.push(removed.id.to_string());
            }
        }
    }

    Ok(PurgeReport {
        dry_run,
        purge_closed_exceptions_after_days: policy.purge_closed_exceptions_after_days,
        exceptions_purged,
        orphan_documents_gc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_documents::{
        attach_document, list_exceptions, raise_exception, set_exception_open, DocumentKind,
        ExceptionSeverity,
    };
    use std::fs;
    use tempfile::tempdir;

    fn write_closed_exception(co: &Path, closed_unix_ms: i64) {
        let exc = raise_exception(
            co,
            "stale".into(),
            ExceptionSeverity::Info,
            "old".into(),
            vec![],
        )
        .unwrap();
        set_exception_open(co, &exc.id, false).unwrap();
        let path = co.join("exceptions.json");
        let mut raw: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        raw["exceptions"][0]["closed_unix_ms"] = serde_json::json!(closed_unix_ms);
        fs::write(path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();
    }

    #[tokio::test]
    async fn dry_run_lists_stale_closed_exceptions() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        crate::save_retention(
            &co,
            &crate::RetentionPolicy {
                retain_days: crate::DEFAULT_RETAIN_DAYS,
                purge_closed_exceptions_after_days: Some(90),
            },
        )
        .unwrap();
        let old = chrono::Utc::now().timestamp_millis() - 100 * MS_PER_DAY;
        write_closed_exception(&co, old);
        let report = run_retention_purge(&co, PurgeOptions::default())
            .await
            .unwrap();
        assert!(report.dry_run);
        assert_eq!(report.exceptions_purged.len(), 1);
        assert_eq!(list_exceptions(&co, false).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn confirm_applies_exception_purge() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        crate::save_retention(
            &co,
            &crate::RetentionPolicy {
                retain_days: crate::DEFAULT_RETAIN_DAYS,
                purge_closed_exceptions_after_days: Some(90),
            },
        )
        .unwrap();
        let old = chrono::Utc::now().timestamp_millis() - 100 * MS_PER_DAY;
        write_closed_exception(&co, old);
        let report = run_retention_purge(
            &co,
            PurgeOptions {
                confirm: true,
                gc_orphan_documents: false,
            },
        )
        .await
        .unwrap();
        assert!(!report.dry_run);
        assert_eq!(report.exceptions_purged.len(), 1);
        assert!(list_exceptions(&co, false).unwrap().is_empty());
    }

    #[tokio::test]
    async fn gc_orphan_document_metadata() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        crate::ensure_retention(&co).unwrap();
        let party = upsert_party(
            &co,
            None,
            "Vendor".into(),
            klarbog_plugin_crm::PartyKind::Private,
            None,
        )
        .unwrap();
        attach_document(
            &co,
            DocumentKind::Receipt,
            "good.pdf".into(),
            Some(party.id.clone()),
            None,
            None,
            None,
        )
        .await
        .unwrap();
        attach_document(
            &co,
            DocumentKind::Other,
            "bad-ref.pdf".into(),
            Some(party.id.clone()),
            None,
            None,
            None,
        )
        .await
        .unwrap();
        fs::write(co.join("parties.json"), r#"{"parties":[]}"#).unwrap();
        let preview = run_retention_purge(
            &co,
            PurgeOptions {
                confirm: false,
                gc_orphan_documents: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(preview.orphan_documents_gc.len(), 2);
        let applied = run_retention_purge(
            &co,
            PurgeOptions {
                confirm: true,
                gc_orphan_documents: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(applied.orphan_documents_gc.len(), 2);
        assert!(klarbog_plugin_documents::list_documents(&co)
            .unwrap()
            .is_empty());
    }
}
