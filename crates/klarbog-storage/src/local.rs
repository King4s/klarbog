use std::path::{Component, Path, PathBuf};

use tokio::fs;

use crate::key::validate_object_key;
use crate::StorageError;

/// Local filesystem object store under an allowlisted absolute root (DEV default, ADR-007).
#[derive(Debug, Clone)]
pub struct LocalFsStore {
    root: PathBuf,
}

impl LocalFsStore {
    /// Create a store at `storage_root`, which must be absolute and under `allowlist_root`.
    pub fn new(allowlist_root: &Path, storage_root: &Path) -> Result<Self, StorageError> {
        if !storage_root.is_absolute() {
            return Err(StorageError::NotAbsolute);
        }
        if storage_root
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(StorageError::ParentDirInRoot);
        }
        let root_canon = allowlist_root
            .canonicalize()
            .unwrap_or_else(|_| allowlist_root.to_path_buf());
        let storage = storage_root.to_path_buf();
        if !storage.starts_with(&root_canon) && !storage.starts_with(allowlist_root) {
            return Err(StorageError::OutsideAllowlist);
        }
        Ok(Self { root: storage })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn resolve(&self, key: &str) -> Result<PathBuf, StorageError> {
        validate_object_key(key)?;
        let joined = self.root.join(key);
        let normalized = normalize_under_root(&self.root, &joined)?;
        Ok(normalized)
    }
}

fn normalize_under_root(root: &Path, candidate: &Path) -> Result<PathBuf, StorageError> {
    let mut out = root.to_path_buf();
    for component in candidate
        .strip_prefix(root)
        .unwrap_or(candidate)
        .components()
    {
        match component {
            Component::ParentDir => return Err(StorageError::PathEscape),
            Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            Component::RootDir | Component::Prefix(_) => {
                return Err(StorageError::PathEscape);
            }
        }
    }
    if !out.starts_with(root) {
        return Err(StorageError::PathEscape);
    }
    Ok(out)
}

impl LocalFsStore {
    pub async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, bytes).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.resolve(key)?;
        match fs::read(&path).await {
            Ok(bytes) => Ok(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(StorageError::NotFound(key.to_string()))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = self.resolve(key)?;
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(StorageError::NotFound(key.to_string()))
            }
            Err(err) => Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn roundtrip_under_allowlist() {
        let dir = tempdir().unwrap();
        let allow = dir.path().to_path_buf();
        let store_root = allow.join("objects");
        fs::create_dir_all(&store_root).await.unwrap();
        let store = LocalFsStore::new(&allow, &store_root).unwrap();

        store.put("a/b.txt", b"hello").await.unwrap();
        assert_eq!(store.get("a/b.txt").await.unwrap(), b"hello");
        store.delete("a/b.txt").await.unwrap();
        assert!(matches!(
            store.get("a/b.txt").await,
            Err(StorageError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn rejects_escape_key() {
        let dir = tempdir().unwrap();
        let allow = dir.path().to_path_buf();
        let store_root = allow.join("objects");
        fs::create_dir_all(&store_root).await.unwrap();
        let store = LocalFsStore::new(&allow, &store_root).unwrap();
        assert_eq!(
            store.put("../escape.txt", b"x").await,
            Err(StorageError::ParentDirInKey)
        );
    }

    #[tokio::test]
    async fn rejects_outside_allowlist() {
        let dir = tempdir().unwrap();
        let allow = dir.path().join("allowed");
        fs::create_dir_all(&allow).await.unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).await.unwrap();
        assert!(matches!(
            LocalFsStore::new(&allow, &outside),
            Err(StorageError::OutsideAllowlist)
        ));
    }
}
