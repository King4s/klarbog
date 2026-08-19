//! HTTP API clients for Revolut and Stripe (primary import path, ADR-008/009).

mod revolut;
mod stripe;

pub use revolut::{fetch_revolut_transactions, parse_revolut_api_json};
pub use stripe::{fetch_stripe_balance_transactions, parse_stripe_api_json};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BankApiError {
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid api response: {0}")]
    Parse(String),
    #[error(transparent)]
    Csv(#[from] crate::csv::BankCsvError),
}

/// Minimal HTTP surface for offline tests with a mock client.
pub trait HttpClient: Send + Sync {
    fn get(
        &self,
        url: &str,
        bearer_token: &str,
    ) -> impl std::future::Future<Output = Result<(u16, String), BankApiError>> + Send;
}

pub struct ReqwestHttpClient;

impl HttpClient for ReqwestHttpClient {
    async fn get(&self, url: &str, bearer_token: &str) -> Result<(u16, String), BankApiError> {
        let resp = reqwest::Client::new()
            .get(url)
            .bearer_auth(bearer_token)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| BankApiError::Request(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| BankApiError::Request(e.to_string()))?;
        Ok((status, body))
    }
}
