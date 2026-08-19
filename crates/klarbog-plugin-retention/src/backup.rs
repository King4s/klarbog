//! Backup manifest writer — file listing + sha256 digests (slice 9).

use crate::digest::sha256_file;
use klarbog_plugin_crm::{list_parties, PARTIES_FILENAME};
use klarbog_plugin_documents::{list_documents, DOCUMENTS_FILENAME, EXCEPTIONS_FILENAME};
use klarbog_plugin_invoice::{list_invoices, INVOICES_FILENAME};
use klarbog_storage::LocalFsStore;
use klarbog_store_sqlite::{open_company, JournalDigestSummary};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const MANIFEST_KEY: &str = "manifest.json";
pub const MANIFEST_SHA256_KEY: &str = "manifest.sha256";

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage: {0}")]
    Storage(#[from] klarbog_storage::StorageError),
    #[error("store: {0}")]
    Store(#[from] klarbog_store_sqlite::StoreError),
    #[error("crm: {0}")]
    Crm(#[from] klarbog_plugin_crm::CrmError),
    #[error("invoice: {0}")]
    Invoice(#[from] klarbog_plugin_invoice::InvoiceError),
    #[error("documents: {0}")]
    Documents(#[from] klarbog_plugin_documents::DocumentError),
    #[error("digest: {0}")]
    Digest(#[from] crate::digest::DigestError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFileEntry {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestPartyRef {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestInvoiceRef {
    pub id: String,
    pub party_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestDocumentRef {
    pub id: String,
    pub path_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub created_unix_ms: i64,
    pub backup_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    pub files: Vec<ManifestFileEntry>,
    pub journal_digests: Vec<JournalDigestSummary>,
    pub parties: Vec<ManifestPartyRef>,
    pub invoices: Vec<ManifestInvoiceRef>,
    pub documents: Vec<ManifestDocumentRef>,
}

const TRACKED_FILES: &[&str] = &[
    "policy.json",
    crate::retention::RETENTION_FILENAME,
    PARTIES_FILENAME,
    INVOICES_FILENAME,
    DOCUMENTS_FILENAME,
    EXCEPTIONS_FILENAME,
    crate::template::TEMPLATE_REL_PATH,
    crate::gdpr::GDPR_EXPORT_FILENAME,
    "ledger.sqlite",
];

fn collect_file_entries(company: &Path) -> Result<Vec<ManifestFileEntry>, BackupError> {
    let mut entries = Vec::new();
    for rel in TRACKED_FILES {
        let path = company.join(rel);
        if !path.is_file() {
            continue;
        }
        let sha256 = sha256_file(&path)?;
        let size_bytes = fs::metadata(&path)?.len();
        entries.push(ManifestFileEntry {
            path: (*rel).to_string(),
            sha256,
            size_bytes,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

/// Build manifest from company dir (no write).
pub async fn build_backup_manifest(company: &Path) -> Result<BackupManifest, BackupError> {
    let store = open_company(company).await?;
    let journal_digests = store.list_journal_digests().await?;
    let files = collect_file_entries(company)?;
    let parties = list_parties(company)?
        .into_iter()
        .map(|p| ManifestPartyRef {
            id: p.id.to_string(),
            display_name: p.display_name,
        })
        .collect();
    let invoices = list_invoices(company)?
        .into_iter()
        .map(|inv| ManifestInvoiceRef {
            id: inv.id.to_string(),
            party_id: inv.party_id.to_string(),
        })
        .collect();
    let documents = list_documents(company)?
        .into_iter()
        .map(|d| ManifestDocumentRef {
            id: d.id.to_string(),
            path_hint: d.path_hint,
        })
        .collect();
    let ts = chrono::Utc::now().timestamp_millis();
    Ok(BackupManifest {
        created_unix_ms: ts,
        backup_key: format!("backups/{ts}/manifest.json"),
        content_sha256: None,
        files,
        journal_digests,
        parties,
        invoices,
        documents,
    })
}

fn sidecar_key_for(manifest_key: &str) -> String {
    manifest_key.replace(MANIFEST_KEY, MANIFEST_SHA256_KEY)
}

/// Path to `manifest.sha256` beside a `manifest.json` path.
pub fn manifest_sidecar_path(manifest_path: &Path) -> PathBuf {
    manifest_path.with_file_name(MANIFEST_SHA256_KEY)
}

/// Verify `manifest.sha256` sidecar matches on-disk `manifest.json` bytes.
pub fn verify_manifest_sidecar(manifest_path: &Path) -> bool {
    let sidecar_path = manifest_sidecar_path(manifest_path);
    let Ok(body) = fs::read(manifest_path) else {
        return false;
    };
    let Ok(expected_raw) = fs::read_to_string(&sidecar_path) else {
        return false;
    };
    let expected = expected_raw.trim();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    crate::digest::sha256_bytes(&body) == expected
}

/// Write manifest to `backups/<ts>/manifest.json` via [`LocalFsStore`].
/// Embeds `content_sha256` (hash of JSON without that field) and writes
/// `manifest.sha256` sidecar (hash of final file bytes).
pub async fn write_backup_manifest(company: &Path) -> Result<BackupManifest, BackupError> {
    let mut manifest = build_backup_manifest(company).await?;
    let key = manifest.backup_key.clone();
    manifest.content_sha256 = None;
    let pre_hash = serde_json::to_string_pretty(&manifest)?;
    manifest.content_sha256 = Some(crate::digest::sha256_bytes(pre_hash.as_bytes()));
    let json = serde_json::to_string_pretty(&manifest)?;
    let store = LocalFsStore::new(company, company)?;
    store.put(&key, json.as_bytes()).await?;
    let file_hash = crate::digest::sha256_bytes(json.as_bytes());
    store
        .put(&sidecar_key_for(&key), format!("{file_hash}\n").as_bytes())
        .await?;
    Ok(manifest)
}

pub fn manifest_path(company: &Path, backup_key: &str) -> PathBuf {
    company.join(backup_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure_company_extras;
    use chrono::Utc;
    use klarbog_core::init_company;
    use klarbog_journal::{Direction, JournalEntry, Leg};
    use klarbog_plugin_crm::upsert_party;
    use klarbog_types::{Actor, Currency, MinorAmount};
    use tempfile::tempdir;

    fn expense(actor: Actor, minor: i64) -> JournalEntry {
        let amount = MinorAmount::from_minor(minor);
        let currency = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: "backup test".into(),
            actor,
            legs: vec![
                Leg {
                    account: "6000".into(),
                    direction: Direction::Debit,
                    amount,
                    currency: currency.clone(),
                    party_id: None,
                },
                Leg {
                    account: "5800".into(),
                    direction: Direction::Credit,
                    amount,
                    currency,
                    party_id: None,
                },
            ],
        }
    }

    #[tokio::test]
    async fn manifest_lists_files_and_digests() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        let actor = Actor::user("owner");
        init_company(&co, "Backup Test ApS", &actor).await.unwrap();
        ensure_company_extras(&co).unwrap();
        upsert_party(&co, None, "Vendor".into()).unwrap();
        let company = klarbog_core::open_existing(&co).await.unwrap();
        let posted = company.post(expense(actor, 500)).await.unwrap();
        assert!(!posted.digest.is_empty());

        let manifest = write_backup_manifest(&co).await.unwrap();
        assert!(manifest.files.iter().any(|f| f.path == "policy.json"));
        assert!(manifest.files.iter().any(|f| f.path == "retention.json"));
        assert_eq!(manifest.journal_digests.len(), 1);
        assert_eq!(manifest.parties.len(), 1);
        let path = manifest_path(&co, &manifest.backup_key);
        assert!(path.exists());
        assert!(manifest
            .content_sha256
            .as_ref()
            .is_some_and(|h| h.len() == 64));
        let sidecar = manifest_sidecar_path(&path);
        assert!(sidecar.exists());
        assert!(verify_manifest_sidecar(&path));
    }

    #[tokio::test]
    async fn sidecar_detects_tamper() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        let actor = Actor::user("owner");
        init_company(&co, "Tamper Test", &actor).await.unwrap();
        ensure_company_extras(&co).unwrap();
        let manifest = write_backup_manifest(&co).await.unwrap();
        let path = manifest_path(&co, &manifest.backup_key);
        assert!(verify_manifest_sidecar(&path));
        let mut text = fs::read_to_string(&path).unwrap();
        text.push(' ');
        fs::write(&path, text).unwrap();
        assert!(!verify_manifest_sidecar(&path));
    }
}
