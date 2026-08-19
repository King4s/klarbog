//! Company-dir path traversal guard (Claude IMPORTANT AuthZ).

use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathGuardError {
    #[error("company path must be absolute")]
    NotAbsolute,
    #[error("company path contains '..'")]
    ParentDir,
    #[error("company path outside allowlist root")]
    OutsideAllowlist,
}

/// Resolve and validate a company directory under an allowlist root.
pub fn assert_company_path(
    allowlist_root: &Path,
    company: &Path,
) -> Result<PathBuf, PathGuardError> {
    if !company.is_absolute() {
        return Err(PathGuardError::NotAbsolute);
    }
    if company
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(PathGuardError::ParentDir);
    }
    let root = allowlist_root
        .canonicalize()
        .unwrap_or_else(|_| allowlist_root.to_path_buf());
    // For not-yet-created dirs, validate prefix logically
    let comp = company.to_path_buf();
    if !comp.starts_with(&root) && !comp.starts_with(allowlist_root) {
        // also allow /tmp for local smoke only when allowlist explicitly includes it
        return Err(PathGuardError::OutsideAllowlist);
    }
    Ok(comp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rejects_dotdot() {
        let root = PathBuf::from("/opt/pellucid-software/klarbog");
        let bad = PathBuf::from("/opt/pellucid-software/klarbog/../seaconnect");
        assert_eq!(
            assert_company_path(&root, &bad),
            Err(PathGuardError::ParentDir)
        );
    }

    #[test]
    fn rejects_relative() {
        let root = PathBuf::from("/opt/pellucid-software/klarbog");
        assert_eq!(
            assert_company_path(&root, Path::new("data/co")),
            Err(PathGuardError::NotAbsolute)
        );
    }
}
