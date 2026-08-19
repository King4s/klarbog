//! Append-only GDPR erase audit trail (no secrets / PII payloads).

use crate::GdprError;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub const GDPR_ERASE_AUDIT_FILENAME: &str = "gdpr_erase_audit.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EraseAuditMode {
    DryRun,
    Confirm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraseAuditLine {
    pub party_id: String,
    pub unix_ms: i64,
    pub mode: EraseAuditMode,
    pub docs_touched: usize,
}

/// Append one JSON line to `{company}/gdpr_erase_audit.jsonl`.
pub fn append_erase_audit(company: &Path, line: &EraseAuditLine) -> Result<(), GdprError> {
    let path = company.join(GDPR_ERASE_AUDIT_FILENAME);
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", serde_json::to_string(line)?)?;
    Ok(())
}

pub fn erase_audit_path(company: &Path) -> std::path::PathBuf {
    company.join(GDPR_ERASE_AUDIT_FILENAME)
}
