//! API credentials from environment (fail closed when missing).

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct RevolutApiConfig {
    pub token: String,
    pub base_url: String,
}

#[derive(Debug, Clone)]
pub struct StripeApiConfig {
    pub secret_key: String,
    pub base_url: String,
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
        let base_url = std::env::var("KLARBOG_REVOLUT_API_BASE")
            .unwrap_or_else(|_| Self::DEFAULT_BASE.to_string());
        Ok(Self { token, base_url })
    }
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

fn required_env(key: &'static str) -> Result<String, BankApiConfigError> {
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
