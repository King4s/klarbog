//! Per-company `retention.json` policy (slice 9).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const RETENTION_FILENAME: &str = "retention.json";

/// Default Danish bookkeeping retention (~5 years).
pub const DEFAULT_RETAIN_DAYS: i64 = 1_825;

#[derive(Debug, Error)]
pub enum RetentionError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("retain_days must be positive")]
    InvalidRetainDays,
    #[error("deadline: {0}")]
    Deadline(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub retain_days: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purge_closed_exceptions_after_days: Option<i64>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            retain_days: DEFAULT_RETAIN_DAYS,
            purge_closed_exceptions_after_days: Some(90),
        }
    }
}

impl RetentionPolicy {
    pub fn validate(&self) -> Result<(), RetentionError> {
        if self.retain_days <= 0 {
            return Err(RetentionError::InvalidRetainDays);
        }
        if let Some(days) = self.purge_closed_exceptions_after_days {
            if days <= 0 {
                return Err(RetentionError::InvalidRetainDays);
            }
        }
        Ok(())
    }
}

fn retention_path(company: &Path) -> PathBuf {
    company.join(RETENTION_FILENAME)
}

pub fn load_retention(company: &Path) -> Result<RetentionPolicy, RetentionError> {
    let path = retention_path(company);
    if !path.exists() {
        return Ok(RetentionPolicy::default());
    }
    let policy: RetentionPolicy = serde_json::from_str(&fs::read_to_string(path)?)?;
    policy.validate()?;
    Ok(policy)
}

pub fn save_retention(company: &Path, policy: &RetentionPolicy) -> Result<(), RetentionError> {
    policy.validate()?;
    let json = serde_json::to_string_pretty(policy)?;
    fs::write(retention_path(company), json)?;
    Ok(())
}

/// Write default retention policy if missing (company init hook).
pub fn ensure_retention(company: &Path) -> Result<RetentionPolicy, RetentionError> {
    let path = retention_path(company);
    if path.exists() {
        return load_retention(company);
    }
    let policy = RetentionPolicy::default();
    save_retention(company, &policy)?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_roundtrip() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let policy = ensure_retention(&co).unwrap();
        assert_eq!(policy.retain_days, DEFAULT_RETAIN_DAYS);
        assert_eq!(policy.purge_closed_exceptions_after_days, Some(90));
        let loaded = load_retention(&co).unwrap();
        assert_eq!(loaded, policy);
    }

    #[test]
    fn rejects_zero_retain_days() {
        let policy = RetentionPolicy {
            retain_days: 0,
            purge_closed_exceptions_after_days: None,
        };
        assert!(matches!(
            policy.validate(),
            Err(RetentionError::InvalidRetainDays)
        ));
    }
}
