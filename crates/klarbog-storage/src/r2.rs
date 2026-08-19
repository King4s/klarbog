use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, HOST};
use reqwest::{Client, StatusCode};

use crate::sigv4::{encode_s3_object_path, sign_s3_request, SignInput};
use crate::validate_object_key;
use crate::StorageError;

const ENV_ACCOUNT_ID: &str = "KLARBOG_R2_ACCOUNT_ID";
const ENV_ACCESS_KEY_ID: &str = "KLARBOG_R2_ACCESS_KEY_ID";
const ENV_SECRET_ACCESS_KEY: &str = "KLARBOG_R2_SECRET_ACCESS_KEY";
const ENV_BUCKET: &str = "KLARBOG_R2_BUCKET";
const ENV_JURISDICTION: &str = "KLARBOG_R2_JURISDICTION";
const ENV_ALLOW_NON_EU: &str = "KLARBOG_R2_ALLOW_NON_EU";

/// S3 region hint for Cloudflare R2 SigV4 (`auto`).
pub const R2_SIGV4_REGION: &str = "auto";

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

    pub fn host(&self) -> Result<String, StorageError> {
        endpoint_host(&self.endpoint)
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

fn endpoint_host(endpoint: &str) -> Result<String, StorageError> {
    let trimmed = endpoint.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme.split('/').next().unwrap_or(without_scheme);
    if host.is_empty() {
        return Err(StorageError::InvalidR2Config(
            "endpoint host must not be empty".into(),
        ));
    }
    Ok(host.to_string())
}

fn amz_date_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_amz_date(secs)
}

fn format_amz_date(unix_secs: u64) -> String {
    // Enough for R2 SigV4; no external chrono dependency.
    let days_since_epoch = unix_secs / 86_400;
    let time_of_day = unix_secs % 86_400;
    let (y, m, d) = civil_from_days(days_since_epoch as i64);
    let hh = time_of_day / 3600;
    let mm = (time_of_day % 3600) / 60;
    let ss = time_of_day % 60;
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

/// R2-backed store with SigV4 PutObject/GetObject (ADR-007).
#[derive(Debug, Clone)]
pub struct R2Store {
    config: R2Config,
    client: Client,
}

impl R2Store {
    pub fn new(config: R2Config) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub fn from_env() -> Result<Self, StorageError> {
        Ok(Self::new(R2Config::from_env()?))
    }

    pub fn config(&self) -> &R2Config {
        &self.config
    }

    fn ensure_jurisdiction(&self) -> Result<(), StorageError> {
        if is_eu_jurisdiction(&self.config.jurisdiction, &self.config.endpoint)
            || self.config.allow_non_eu
        {
            Ok(())
        } else {
            Err(StorageError::NonEuJurisdiction)
        }
    }

    fn object_url(&self, key: &str) -> String {
        let uri = encode_s3_object_path(&self.config.bucket, key);
        format!("{}{uri}", self.config.endpoint.trim_end_matches('/'))
    }

    fn sign_for(
        &self,
        method: &str,
        key: &str,
        payload: &[u8],
        amz_date: &str,
    ) -> Result<(String, crate::sigv4::SignedHeaders), StorageError> {
        let host = self.config.host()?;
        let uri = encode_s3_object_path(&self.config.bucket, key);
        let signed = sign_s3_request(&SignInput {
            method,
            host: &host,
            canonical_uri: &uri,
            payload,
            access_key_id: &self.config.access_key_id,
            secret_access_key: &self.config.secret_access_key,
            region: R2_SIGV4_REGION,
            amz_date,
        });
        Ok((self.object_url(key), signed))
    }

    pub async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        validate_object_key(key)?;
        self.ensure_jurisdiction()?;
        let amz_date = amz_date_now();
        let (url, signed) = self.sign_for("PUT", key, bytes, &amz_date)?;
        let resp = self
            .client
            .put(url)
            .header(HOST, &signed.host)
            .header("x-amz-content-sha256", &signed.x_amz_content_sha256)
            .header("x-amz-date", &signed.x_amz_date)
            .header(AUTHORIZATION, &signed.authorization)
            .header(CONTENT_LENGTH, bytes.len())
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|e| StorageError::Http(e.to_string()))?;
        map_s3_response(resp).await
    }

    pub async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        validate_object_key(key)?;
        self.ensure_jurisdiction()?;
        let amz_date = amz_date_now();
        let (url, signed) = self.sign_for("GET", key, b"", &amz_date)?;
        let resp = self
            .client
            .get(url)
            .header(HOST, &signed.host)
            .header("x-amz-content-sha256", &signed.x_amz_content_sha256)
            .header("x-amz-date", &signed.x_amz_date)
            .header(AUTHORIZATION, &signed.authorization)
            .send()
            .await
            .map_err(|e| StorageError::Http(e.to_string()))?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Err(StorageError::NotFound(key.to_string()));
        }
        let status = resp.status();
        let body = resp
            .bytes()
            .await
            .map_err(|e| StorageError::Http(e.to_string()))?;
        if status.is_success() {
            Ok(body.to_vec())
        } else {
            Err(StorageError::R2Response {
                status: status.as_u16(),
                body: String::from_utf8_lossy(&body).into_owned(),
            })
        }
    }

    pub async fn delete(&self, _key: &str) -> Result<(), StorageError> {
        Err(StorageError::NotImplementedInDev)
    }
}

async fn map_s3_response(resp: reqwest::Response) -> Result<(), StorageError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let body = resp
        .bytes()
        .await
        .map_err(|e| StorageError::Http(e.to_string()))?;
    if status == StatusCode::NOT_FOUND {
        return Err(StorageError::NotFound(String::new()));
    }
    Err(StorageError::R2Response {
        status: status.as_u16(),
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sigv4::{encode_s3_object_path, sign_s3_request, SignInput};

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

    fn eu_config() -> R2Config {
        let (a, k, s, b, j, allow) = sample_parts("eu", false);
        R2Config::from_parts(a, k, s, b, j, allow).unwrap()
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

    #[test]
    fn object_url_and_signing_offline() {
        let store = R2Store::new(eu_config());
        let (url, signed) = store
            .sign_for("PUT", "attachments/x.pdf", b"data", "20260820T100000Z")
            .unwrap();
        assert_eq!(
            url,
            "https://acct.eu.r2.cloudflarestorage.com/bucket/attachments/x.pdf"
        );
        assert!(signed
            .authorization
            .contains("Credential=key-id/20260820/auto/s3/aws4_request"));
    }

    #[test]
    fn format_amz_date_known_epoch() {
        assert_eq!(format_amz_date(1_787_220_000), "20260820T100000Z");
    }

    #[tokio::test]
    async fn put_rejects_invalid_key_without_network() {
        let store = R2Store::new(eu_config());
        assert_eq!(
            store.put("../escape", b"x").await,
            Err(StorageError::ParentDirInKey)
        );
    }

    #[tokio::test]
    async fn get_rejects_empty_key_without_network() {
        let store = R2Store::new(eu_config());
        assert_eq!(store.get("  ").await, Err(StorageError::EmptyKey));
    }

    #[test]
    fn canonical_uri_matches_sigv4_module() {
        let cfg = eu_config();
        let host = cfg.host().unwrap();
        let uri = encode_s3_object_path(&cfg.bucket, "attachments/a b.pdf");
        let signed = sign_s3_request(&SignInput {
            method: "PUT",
            host: &host,
            canonical_uri: &uri,
            payload: b"hello",
            access_key_id: &cfg.access_key_id,
            secret_access_key: &cfg.secret_access_key,
            region: R2_SIGV4_REGION,
            amz_date: "20260820T100000Z",
        });
        assert!(signed.authorization.starts_with("AWS4-HMAC-SHA256"));
    }
}
