//! Cloudflare R2 config + Put/Get/Delete (ADR-007).

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

use crate::r2_time::amz_date_now;

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

    pub(crate) fn sign_for(
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

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        validate_object_key(key)?;
        self.ensure_jurisdiction()?;
        let amz_date = amz_date_now();
        let (url, signed) = self.sign_for("DELETE", key, b"", &amz_date)?;
        let resp = self
            .client
            .delete(url)
            .header(HOST, &signed.host)
            .header("x-amz-content-sha256", &signed.x_amz_content_sha256)
            .header("x-amz-date", &signed.x_amz_date)
            .header(AUTHORIZATION, &signed.authorization)
            .send()
            .await
            .map_err(|e| StorageError::Http(e.to_string()))?;
        map_s3_response(resp).await
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
