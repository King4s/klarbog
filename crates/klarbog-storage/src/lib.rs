//! Object storage for Klarbog attachments (ADR-007).
//!
//! DEV default: [`LocalFsStore`] under the company allowlist path.
//! Stage/prod may use [`R2Store`] (Cloudflare R2 EU) with SigV4 PutObject/GetObject.

mod error;
mod key;
mod local;
mod r2;
#[cfg(test)]
mod r2_tests;
mod r2_time;
mod select;
mod sigv4;

pub use error::StorageError;
pub use key::validate_object_key;
pub use local::LocalFsStore;
pub use r2::{build_r2_endpoint, is_eu_jurisdiction, R2Config, R2Store};
pub use select::{
    company_objects_root, ensure_company_objects_root, klarbog_storage, KlarbogStorage,
    COMPANY_OBJECTS_DIR, ENV_KLARBOG_STORAGE,
};

/// Async object store: put/get/delete by relative key (no `..`).
#[allow(async_fn_in_trait)]
pub trait ObjectStore: Send + Sync {
    fn put(
        &self,
        key: &str,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<(), StorageError>> + Send;
    fn get(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, StorageError>> + Send;
    fn delete(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<(), StorageError>> + Send;
}

impl ObjectStore for LocalFsStore {
    fn put(
        &self,
        key: &str,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<(), StorageError>> + Send {
        LocalFsStore::put(self, key, bytes)
    }

    fn get(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, StorageError>> + Send {
        LocalFsStore::get(self, key)
    }

    fn delete(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<(), StorageError>> + Send {
        LocalFsStore::delete(self, key)
    }
}

impl ObjectStore for R2Store {
    fn put(
        &self,
        key: &str,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<(), StorageError>> + Send {
        R2Store::put(self, key, bytes)
    }

    fn get(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, StorageError>> + Send {
        R2Store::get(self, key)
    }

    fn delete(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<(), StorageError>> + Send {
        R2Store::delete(self, key)
    }
}
