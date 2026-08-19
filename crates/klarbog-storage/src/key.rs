use std::path::{Component, Path};

use crate::StorageError;

/// Validate an object key: relative, non-empty, no `..` segments.
pub fn validate_object_key(key: &str) -> Result<(), StorageError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(StorageError::EmptyKey);
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(StorageError::AbsoluteKey);
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(StorageError::ParentDirInKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dotdot() {
        assert_eq!(
            validate_object_key("attachments/../secret.pdf"),
            Err(StorageError::ParentDirInKey)
        );
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(validate_object_key("  "), Err(StorageError::EmptyKey));
    }

    #[test]
    fn rejects_absolute() {
        assert_eq!(
            validate_object_key("/abs/key"),
            Err(StorageError::AbsoluteKey)
        );
    }

    #[test]
    fn accepts_relative() {
        assert!(validate_object_key("attachments/2026/receipt.pdf").is_ok());
    }
}
