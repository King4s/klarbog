//! Per-company Revolut OAuth token persistence (`secrets/revolut.json`, mode 0600).

use crate::config::BankApiConfigError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const REVOLUT_SECRETS_DIR: &str = "secrets";
pub const REVOLUT_TOKENS_FILENAME: &str = "revolut.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevolutStoredTokens {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Unix epoch milliseconds when `access_token` expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
}

#[derive(Debug, Error)]
pub enum RevolutTokenStoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid token file: {0}")]
    Parse(String),
    #[error("stored access token is empty")]
    EmptyAccessToken,
}

pub fn revolut_tokens_path(company: &Path) -> PathBuf {
    company
        .join(REVOLUT_SECRETS_DIR)
        .join(REVOLUT_TOKENS_FILENAME)
}

pub fn load_revolut_tokens(company: &Path) -> Result<RevolutStoredTokens, RevolutTokenStoreError> {
    let path = revolut_tokens_path(company);
    let raw = fs::read_to_string(&path)?;
    let tokens: RevolutStoredTokens =
        serde_json::from_str(&raw).map_err(|e| RevolutTokenStoreError::Parse(e.to_string()))?;
    if tokens.access_token.trim().is_empty() {
        return Err(RevolutTokenStoreError::EmptyAccessToken);
    }
    Ok(tokens)
}

pub fn save_revolut_tokens(
    company: &Path,
    tokens: &RevolutStoredTokens,
) -> Result<(), RevolutTokenStoreError> {
    let dir = company.join(REVOLUT_SECRETS_DIR);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(&dir)?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(&dir)?;
    }
    let path = revolut_tokens_path(company);
    let json = serde_json::to_string_pretty(tokens)
        .map_err(|e| RevolutTokenStoreError::Parse(e.to_string()))?;
    write_secret_file_atomic(&path, &json)?;
    Ok(())
}

fn write_secret_file_atomic(path: &Path, contents: &str) -> Result<(), RevolutTokenStoreError> {
    let dir = path.parent().ok_or_else(|| {
        RevolutTokenStoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "token path has no parent",
        ))
    })?;
    let tmp = dir.join(format!(
        ".{}.tmp.{}",
        REVOLUT_TOKENS_FILENAME,
        std::process::id()
    ));
    let write_result = (|| -> Result<(), RevolutTokenStoreError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&tmp, contents)?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write_result
}

pub fn load_revolut_access_token(company: &Path) -> Result<String, BankApiConfigError> {
    load_revolut_tokens(company)
        .map(|t| t.access_token)
        .map_err(|_| BankApiConfigError::MissingEnv("KLARBOG_REVOLUT_API_TOKEN"))
}

pub fn access_token_expired(tokens: &RevolutStoredTokens, now_ms: i64) -> bool {
    tokens
        .expires_at
        .is_some_and(|expires_at| now_ms >= expires_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_tokens_under_secrets() {
        let dir = tempdir().unwrap();
        let company = dir.path().join("co");
        fs::create_dir_all(&company).unwrap();
        let tokens = RevolutStoredTokens {
            access_token: "access-test".into(),
            refresh_token: Some("refresh-test".into()),
            expires_at: Some(1_700_000_000_000),
            token_type: Some("Bearer".into()),
        };
        save_revolut_tokens(&company, &tokens).unwrap();
        let loaded = load_revolut_tokens(&company).unwrap();
        assert_eq!(loaded, tokens);
        assert_eq!(
            revolut_tokens_path(&company),
            company.join("secrets/revolut.json")
        );
    }

    #[cfg(unix)]
    #[test]
    fn secrets_file_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let company = dir.path().join("co");
        fs::create_dir_all(&company).unwrap();
        let tokens = RevolutStoredTokens {
            access_token: "x".into(),
            refresh_token: None,
            expires_at: None,
            token_type: None,
        };
        save_revolut_tokens(&company, &tokens).unwrap();
        let mode = fs::metadata(revolut_tokens_path(&company))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn expired_helper_uses_unix_ms() {
        let tokens = RevolutStoredTokens {
            access_token: "x".into(),
            refresh_token: Some("r".into()),
            expires_at: Some(1_000),
            token_type: None,
        };
        assert!(access_token_expired(&tokens, 1_000));
        assert!(!access_token_expired(&tokens, 999));
    }
}
