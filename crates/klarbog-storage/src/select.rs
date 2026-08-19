//! Storage backend selection from `KLARBOG_STORAGE` (ADR-007).

use std::path::{Path, PathBuf};

use crate::{LocalFsStore, R2Config, R2Store, StorageError};

pub const ENV_KLARBOG_STORAGE: &str = "KLARBOG_STORAGE";
pub const COMPANY_OBJECTS_DIR: &str = "objects";

/// Active object storage backend for the current process.
#[derive(Debug, Clone)]
pub enum KlarbogStorage {
    Local(LocalFsStore),
    R2(R2Store),
}

impl KlarbogStorage {
    /// Put bytes at `key` using the active backend (local `objects/` or R2).
    pub async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        match self {
            Self::Local(store) => store.put(key, bytes).await,
            Self::R2(store) => store.put(key, bytes).await,
        }
    }

    /// Delete object at `key` using the active backend.
    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match self {
            Self::Local(store) => store.delete(key).await,
            Self::R2(store) => store.delete(key).await,
        }
    }
}

/// Select storage from `KLARBOG_STORAGE` (default `local`).
///
/// - `local`: ensure `<company>/objects/` exists; return [`LocalFsStore`] rooted there.
/// - `r2`: validate Cloudflare R2 env via [`R2Config::from_env`] (no local dir).
pub fn klarbog_storage(company: &Path) -> Result<KlarbogStorage, StorageError> {
    match storage_backend_from_env() {
        StorageBackend::Local => {
            let root = ensure_company_objects_root(company)?;
            Ok(KlarbogStorage::Local(LocalFsStore::new(company, &root)?))
        }
        StorageBackend::R2 => Ok(KlarbogStorage::R2(R2Store::new(R2Config::from_env()?))),
    }
}

pub fn company_objects_root(company: &Path) -> PathBuf {
    company.join(COMPANY_OBJECTS_DIR)
}

/// Create `<company>/objects/` when missing (local DEV default).
pub fn ensure_company_objects_root(company: &Path) -> Result<PathBuf, StorageError> {
    let root = company_objects_root(company);
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageBackend {
    Local,
    R2,
}

fn storage_backend_from_env() -> StorageBackend {
    match std::env::var(ENV_KLARBOG_STORAGE) {
        Ok(v) if v.trim().eq_ignore_ascii_case("r2") => StorageBackend::R2,
        _ => StorageBackend::Local,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_r2_env() {
        std::env::remove_var(ENV_KLARBOG_STORAGE);
        std::env::remove_var("KLARBOG_R2_ACCOUNT_ID");
        std::env::remove_var("KLARBOG_R2_ACCESS_KEY_ID");
        std::env::remove_var("KLARBOG_R2_SECRET_ACCESS_KEY");
        std::env::remove_var("KLARBOG_R2_BUCKET");
        std::env::remove_var("KLARBOG_R2_JURISDICTION");
        std::env::remove_var("KLARBOG_R2_ALLOW_NON_EU");
    }

    #[test]
    fn defaults_to_local_and_creates_objects_dir() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_r2_env();
        let dir = tempdir().unwrap();
        let company = dir.path().join("co");
        std::fs::create_dir_all(&company).unwrap();
        let sel = klarbog_storage(&company).unwrap();
        assert!(matches!(sel, KlarbogStorage::Local(_)));
        assert!(company.join(COMPANY_OBJECTS_DIR).is_dir());
    }

    #[test]
    fn r2_mode_requires_env() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_r2_env();
        let dir = tempdir().unwrap();
        let company = dir.path().join("co");
        std::fs::create_dir_all(&company).unwrap();
        std::env::set_var(ENV_KLARBOG_STORAGE, "r2");
        let err = klarbog_storage(&company).unwrap_err();
        clear_r2_env();
        assert!(matches!(err, StorageError::MissingEnv(_)));
    }

    #[tokio::test]
    async fn local_delete_roundtrip() {
        let (dir, backend) = {
            let _g = ENV_LOCK.lock().unwrap();
            clear_r2_env();
            let dir = tempdir().unwrap();
            let company = dir.path().join("co");
            std::fs::create_dir_all(&company).unwrap();
            let backend = klarbog_storage(&company).unwrap();
            (dir, backend)
        };
        backend.put("a/x.bin", b"data").await.unwrap();
        backend.delete("a/x.bin").await.unwrap();
        assert!(matches!(
            backend.delete("a/x.bin").await,
            Err(StorageError::NotFound(_))
        ));
        drop(dir);
    }

    #[test]
    fn r2_mode_selects_r2_store_when_env_present() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_r2_env();
        let dir = tempdir().unwrap();
        let company = dir.path().join("co");
        std::fs::create_dir_all(&company).unwrap();
        std::env::set_var(ENV_KLARBOG_STORAGE, "r2");
        std::env::set_var("KLARBOG_R2_ACCOUNT_ID", "acct");
        std::env::set_var("KLARBOG_R2_ACCESS_KEY_ID", "key");
        std::env::set_var("KLARBOG_R2_SECRET_ACCESS_KEY", "secret");
        std::env::set_var("KLARBOG_R2_BUCKET", "bucket");
        let sel = klarbog_storage(&company).unwrap();
        clear_r2_env();
        assert!(matches!(sel, KlarbogStorage::R2(_)));
    }
}
