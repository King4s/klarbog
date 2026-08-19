use crate::StorageError;

const ENV_ACCOUNT_ID: &str = "KLARBOG_R2_ACCOUNT_ID";
const ENV_ACCESS_KEY_ID: &str = "KLARBOG_R2_ACCESS_KEY_ID";
const ENV_SECRET_ACCESS_KEY: &str = "KLARBOG_R2_SECRET_ACCESS_KEY";
const ENV_BUCKET: &str = "KLARBOG_R2_BUCKET";
const ENV_JURISDICTION: &str = "KLARBOG_R2_JURISDICTION";
const ENV_ALLOW_NON_EU: &str = "KLARBOG_R2_ALLOW_NON_EU";

/// Cloudflare R2 connection settings (env-only credentials, ADR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R2Config {
    pub account_id: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub jurisdiction: String,
    pub endpoint: String,
    pub allow_non_eu: bool,
}

impl R2Config {
    /// Load from process environment. Fails closed on non-EU unless override env is set.
    pub fn from_env() -> Result<Self, StorageError> {
        let account_id = required_env(ENV_ACCOUNT_ID)?;
        let access_key_id = required_env(ENV_ACCESS_KEY_ID)?;
        let secret_access_key = required_env(ENV_SECRET_ACCESS_KEY)?;
        let bucket = required_env(ENV_BUCKET)?;
        let jurisdiction = std::env::var(ENV_JURISDICTION).unwrap_or_else(|_| "eu".into());
        let allow_non_eu = env_truthy(ENV_ALLOW_NON_EU);
        Self::from_parts(
            account_id,
            access_key_id,
            secret_access_key,
            bucket,
            jurisdiction,
            allow_non_eu,
        )
    }

    pub fn from_parts(
        account_id: String,
        access_key_id: String,
        secret_access_key: String,
        bucket: String,
        jurisdiction: String,
        allow_non_eu: bool,
    ) -> Result<Self, StorageError> {
        if account_id.trim().is_empty() {
            return Err(StorageError::InvalidR2Config(
                "account_id must not be empty".into(),
            ));
        }
        if bucket.trim().is_empty() {
            return Err(StorageError::InvalidR2Config(
                "bucket must not be empty".into(),
            ));
        }
        let endpoint = build_r2_endpoint(&account_id, &jurisdiction);
        let eu = is_eu_jurisdiction(&jurisdiction, &endpoint);
        if !eu && !allow_non_eu {
            return Err(StorageError::NonEuJurisdiction);
        }
        Ok(Self {
            account_id,
            access_key_id,
            secret_access_key,
            bucket,
            jurisdiction,
            endpoint,
            allow_non_eu,
        })
    }
}

/// S3-compatible endpoint for Cloudflare R2.
pub fn build_r2_endpoint(account_id: &str, jurisdiction: &str) -> String {
    let j = jurisdiction.trim().to_ascii_lowercase();
    if j.is_empty() || j == "eu" || j == "weur" {
        format!("https://{account_id}.eu.r2.cloudflarestorage.com")
    } else {
        format!("https://{account_id}.r2.cloudflarestorage.com")
    }
}

pub fn is_eu_jurisdiction(jurisdiction: &str, endpoint: &str) -> bool {
    let j = jurisdiction.trim().to_ascii_lowercase();
    if j == "eu" || j == "weur" {
        return true;
    }
    endpoint.contains(".eu.r2.cloudflarestorage.com")
}

fn required_env(name: &'static str) -> Result<String, StorageError> {
    std::env::var(name).map_err(|_| StorageError::MissingEnv(name))
}

fn env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let lower = v.trim().to_ascii_lowercase();
            lower == "1" || lower == "true" || lower == "yes"
        }
        Err(_) => false,
    }
}

/// R2-backed store scaffold — validates config; network I/O deferred (ADR-007).
#[derive(Debug, Clone)]
pub struct R2Store {
    config: R2Config,
}

impl R2Store {
    pub fn new(config: R2Config) -> Self {
        Self { config }
    }

    pub fn from_env() -> Result<Self, StorageError> {
        Ok(Self::new(R2Config::from_env()?))
    }

    pub fn config(&self) -> &R2Config {
        &self.config
    }

    pub async fn put(&self, _key: &str, _bytes: &[u8]) -> Result<(), StorageError> {
        let _ = &self.config;
        Err(StorageError::NotImplementedInDev)
    }

    pub async fn get(&self, _key: &str) -> Result<Vec<u8>, StorageError> {
        let _ = &self.config;
        Err(StorageError::NotImplementedInDev)
    }

    pub async fn delete(&self, _key: &str) -> Result<(), StorageError> {
        let _ = &self.config;
        Err(StorageError::NotImplementedInDev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_parts(
        jurisdiction: &str,
        allow_non_eu: bool,
    ) -> (String, String, String, String, String, bool) {
        (
            "acct".into(),
            "key-id".into(),
            "secret".into(),
            "bucket".into(),
            jurisdiction.into(),
            allow_non_eu,
        )
    }

    #[test]
    fn eu_endpoint_default_jurisdiction() {
        let ep = build_r2_endpoint("abc123", "eu");
        assert_eq!(ep, "https://abc123.eu.r2.cloudflarestorage.com");
        assert!(is_eu_jurisdiction("eu", &ep));
    }

    #[test]
    fn non_eu_rejected_without_override() {
        let (a, k, s, b, j, allow) = sample_parts("us", false);
        assert_eq!(
            R2Config::from_parts(a, k, s, b, j, allow),
            Err(StorageError::NonEuJurisdiction)
        );
    }

    #[test]
    fn non_eu_allowed_with_override_flag() {
        let (a, k, s, b, j, allow) = sample_parts("us", true);
        let cfg = R2Config::from_parts(a, k, s, b, j, allow).unwrap();
        assert!(!is_eu_jurisdiction(&cfg.jurisdiction, &cfg.endpoint));
        assert!(cfg.allow_non_eu);
    }

    #[test]
    fn empty_bucket_rejected() {
        let err = R2Config::from_parts(
            "acct".into(),
            "k".into(),
            "s".into(),
            "  ".into(),
            "eu".into(),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, StorageError::InvalidR2Config(_)));
    }

    #[tokio::test]
    async fn r2_store_stub_returns_not_implemented() {
        let (a, k, s, b, j, allow) = sample_parts("eu", false);
        let cfg = R2Config::from_parts(a, k, s, b, j, allow).unwrap();
        let store = R2Store::new(cfg);
        assert_eq!(
            store.put("k", b"x").await,
            Err(StorageError::NotImplementedInDev)
        );
    }
}
