//! API credentials from environment (fail closed when missing).

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct RevolutApiConfig {
    pub token: String,
    pub base_url: String,
}

#[derive(Debug, Clone)]
pub struct RevolutOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub token_url: String,
    pub auth_url: String,
}

#[derive(Debug, Clone)]
pub struct StripeApiConfig {
    pub secret_key: String,
    pub base_url: String,
}

#[derive(Debug, Clone)]
pub struct StripeWebhookConfig {
    pub webhook_secret: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankApiConfigError {
    #[error("missing env: {0}")]
    MissingEnv(&'static str),
}

impl RevolutApiConfig {
    pub const DEFAULT_BASE: &'static str = "https://b2b.revolut.com/api/1.0";

    pub fn from_env() -> Result<Self, BankApiConfigError> {
        let token = required_env("KLARBOG_REVOLUT_API_TOKEN")?;
        Ok(Self {
            token,
            base_url: revolut_api_base_from_env(),
        })
    }

    /// Env bearer token first; else company `secrets/revolut.json` access token (OAuth scaffold).
    pub fn from_env_or_company_secrets(company: &Path) -> Result<Self, BankApiConfigError> {
        if let Ok(cfg) = Self::from_env() {
            return Ok(cfg);
        }
        let token = crate::oauth::load_revolut_access_token(company)?;
        Ok(Self {
            token,
            base_url: revolut_api_base_from_env(),
        })
    }
}

impl RevolutOAuthConfig {
    pub const DEFAULT_TOKEN_URL: &'static str = "https://b2b.revolut.com/api/1.0/auth/token";

    pub fn from_env() -> Result<Self, BankApiConfigError> {
        Ok(Self {
            client_id: required_env("KLARBOG_REVOLUT_CLIENT_ID")?,
            client_secret: required_env("KLARBOG_REVOLUT_CLIENT_SECRET")?,
            redirect_uri: required_env("KLARBOG_REVOLUT_REDIRECT_URI")?,
            token_url: std::env::var("KLARBOG_REVOLUT_TOKEN_URL")
                .unwrap_or_else(|_| Self::DEFAULT_TOKEN_URL.to_string()),
            auth_url: std::env::var("KLARBOG_REVOLUT_AUTH_URL")
                .unwrap_or_else(|_| crate::oauth::DEFAULT_AUTH_URL.to_string()),
        })
    }
}

fn revolut_api_base_from_env() -> String {
    std::env::var("KLARBOG_REVOLUT_API_BASE")
        .unwrap_or_else(|_| RevolutApiConfig::DEFAULT_BASE.to_string())
}

impl StripeApiConfig {
    pub const DEFAULT_BASE: &'static str = "https://api.stripe.com";

    pub fn from_env() -> Result<Self, BankApiConfigError> {
        let secret_key = required_env("KLARBOG_STRIPE_SECRET_KEY")?;
        let base_url = std::env::var("KLARBOG_STRIPE_API_BASE")
            .unwrap_or_else(|_| Self::DEFAULT_BASE.to_string());
        Ok(Self {
            secret_key,
            base_url,
        })
    }
}

impl StripeWebhookConfig {
    pub fn from_env() -> Result<Self, BankApiConfigError> {
        Ok(Self {
            webhook_secret: required_env("KLARBOG_STRIPE_WEBHOOK_SECRET")?,
        })
    }
}

pub(crate) fn required_env(key: &'static str) -> Result<String, BankApiConfigError> {
    let value = std::env::var(key).map_err(|_| BankApiConfigError::MissingEnv(key))?;
    if value.trim().is_empty() {
        return Err(BankApiConfigError::MissingEnv(key));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revolut_from_env_requires_token() {
        let _guard = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
        let err = RevolutApiConfig::from_env().unwrap_err();
        assert_eq!(
            err,
            BankApiConfigError::MissingEnv("KLARBOG_REVOLUT_API_TOKEN")
        );
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            // SAFETY: test-only single-threaded env mutation.
            unsafe { std::env::remove_var(key) };
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }
}
