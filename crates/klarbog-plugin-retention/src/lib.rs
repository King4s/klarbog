//! Retention, backup manifest, GDPR export v1, templates (slice 9+18).
//! No journal-write capability (ADR-004). Reuses [`LocalFsStore`] (ADR-007).

mod backup;
mod digest;
mod gdpr;
mod purge;
mod retention;
mod template;

pub use backup::{
    build_backup_manifest, manifest_path, manifest_sidecar_path, verify_manifest_sidecar,
    write_backup_manifest, BackupError, BackupManifest, ManifestDocumentRef, ManifestFileEntry,
    ManifestInvoiceRef, ManifestPartyRef, MANIFEST_KEY, MANIFEST_SHA256_KEY,
};
pub use gdpr::{
    build_gdpr_export, gdpr_export_path, write_gdpr_export, GdprDocument, GdprError, GdprException,
    GdprExport, GdprInvoice, GdprParty, GdprRetentionSummary, GDPR_EXPORT_FILENAME,
};
pub use purge::{run_retention_purge, PurgeError, PurgeOptions, PurgeReport};
pub use retention::{
    ensure_retention, load_retention, save_retention, RetentionError, RetentionPolicy,
    DEFAULT_RETAIN_DAYS, RETENTION_FILENAME,
};
pub use template::{ensure_expense_memo_template, EXPENSE_MEMO_TEMPLATE, TEMPLATE_REL_PATH};

use klarbog_plugin::{Capability, Plugin};
use std::path::Path;

pub struct RetentionPlugin;

impl Default for RetentionPlugin {
    fn default() -> Self {
        Self
    }
}

/// Company init hook: retention policy + expense memo template.
pub fn ensure_company_extras(company: &Path) -> Result<(), RetentionError> {
    ensure_retention(company)?;
    ensure_expense_memo_template(company).map_err(|e| match e {
        crate::template::TemplateError::Io(err) => RetentionError::Io(err),
    })?;
    Ok(())
}

impl Plugin for RetentionPlugin {
    fn id(&self) -> &'static str {
        "retention"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::Read]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plugin_has_no_journal_write() {
        assert!(!RetentionPlugin.has_journal_write());
    }

    #[test]
    fn init_extras_writes_retention_and_template() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        ensure_company_extras(&co).unwrap();
        assert!(co.join(RETENTION_FILENAME).exists());
        assert!(co.join(TEMPLATE_REL_PATH).exists());
    }
}
