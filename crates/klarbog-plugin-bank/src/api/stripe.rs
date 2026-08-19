//! Stripe balance-transactions API fetch + JSON parse (ADR-009).

use crate::api::{BankApiError, HttpClient, ReqwestHttpClient};
use crate::config::StripeApiConfig;
use crate::csv::{validate_currencies, BankCsvError, BankRow};
use chrono::{DateTime, Utc};
use klarbog_types::{Currency, MinorAmount};
use serde_json::Value;

pub async fn fetch_stripe_balance_transactions(
    cfg: &StripeApiConfig,
    starting_after: Option<&str>,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankApiError> {
    fetch_stripe_balance_transactions_with_client(
        &ReqwestHttpClient,
        cfg,
        starting_after,
        required_currency,
    )
    .await
}

pub async fn fetch_stripe_balance_transactions_with_client<C: HttpClient>(
    client: &C,
    cfg: &StripeApiConfig,
    starting_after: Option<&str>,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankApiError> {
    let mut url = format!(
        "{}/v1/balance_transactions?limit=100",
        cfg.base_url.trim_end_matches('/')
    );
    if let Some(after) = starting_after {
        url.push_str(&format!("&starting_after={after}"));
    }
    let (status, body) = client.get(&url, &cfg.secret_key).await?;
    if status != 200 {
        return Err(BankApiError::Http { status, body });
    }
    Ok(parse_stripe_api_json(&body, required_currency)?)
}

pub fn parse_stripe_api_json(
    json: &str,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankCsvError> {
    let root: Value = serde_json::from_str(json).map_err(|e| BankCsvError::Row {
        row: 0,
        detail: format!("invalid json: {e}"),
    })?;
    let items = root
        .get("data")
        .and_then(|v| v.as_array())
        .or_else(|| root.as_array())
        .ok_or(BankCsvError::Row {
            row: 0,
            detail: "expected data array".into(),
        })?;

    let mut out = Vec::new();
    let mut currencies = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let row_no = i + 1;
        let created = item
            .get("created")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| BankCsvError::Row {
                row: row_no,
                detail: "missing created".into(),
            })?;
        let date = DateTime::from_timestamp(created, 0).ok_or_else(|| BankCsvError::Row {
            row: row_no,
            detail: format!("invalid created timestamp: {created}"),
        })?;

        let text = stripe_description(item);
        let currency_raw = item
            .get("currency")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BankCsvError::Row {
                row: row_no,
                detail: "missing currency".into(),
            })?;
        let code = currency_raw.trim().to_uppercase();
        Currency::new(&code).map_err(|_| BankCsvError::Row {
            row: row_no,
            detail: format!("invalid currency: {currency_raw}"),
        })?;
        currencies.push(code);

        let minor = stripe_net_minor(item, row_no)?;
        out.push(BankRow {
            date: date.with_timezone(&Utc),
            text,
            amount_minor: MinorAmount::from_minor(minor),
        });
    }

    validate_currencies(&currencies, required_currency)?;
    Ok(out)
}

fn stripe_description(item: &Value) -> String {
    item.get("description")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("type").and_then(|v| v.as_str()))
        .unwrap_or("Stripe transaction")
        .to_string()
}

fn stripe_net_minor(item: &Value, row_no: usize) -> Result<i64, BankCsvError> {
    let net = item
        .get("net")
        .and_then(|v| v.as_i64())
        .or_else(|| item.get("amount").and_then(|v| v.as_i64()))
        .ok_or_else(|| BankCsvError::Row {
            row: row_no,
            detail: "missing net/amount".into(),
        })?;
    Ok(net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::HttpClient;

    const FIXTURE: &str = include_str!("../../tests/fixtures/stripe_api.json");

    struct MockClient {
        body: String,
    }

    impl HttpClient for MockClient {
        async fn get(
            &self,
            _url: &str,
            _bearer_token: &str,
        ) -> Result<(u16, String), BankApiError> {
            Ok((200, self.body.clone()))
        }

        async fn post_form(
            &self,
            _url: &str,
            _body: &str,
            _bearer_token: Option<&str>,
        ) -> Result<(u16, String), BankApiError> {
            Ok((200, "{}".into()))
        }
    }

    #[test]
    fn parse_fixture_prefers_net() {
        let rows = parse_stripe_api_json(FIXTURE, Some(&Currency::new("DKK").unwrap())).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].text, "Stripe payout");
        assert_eq!(rows[0].amount_minor.minor(), -100000);
        assert_eq!(rows[1].amount_minor.minor(), 24275);
        assert_eq!(rows[2].amount_minor.minor(), -5000);
    }

    #[tokio::test]
    async fn fetch_with_mock_client() {
        let cfg = StripeApiConfig {
            secret_key: "sk_test".into(),
            base_url: "https://api.stripe.com".into(),
        };
        let client = MockClient {
            body: FIXTURE.to_string(),
        };
        let rows = fetch_stripe_balance_transactions_with_client(
            &client,
            &cfg,
            None,
            Some(&Currency::new("DKK").unwrap()),
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn rejects_mixed_currency() {
        let json = r#"{"data":[
          {"created":1735689600,"description":"A","net":-1000,"currency":"dkk"},
          {"created":1735776000,"description":"B","net":500,"currency":"eur"}
        ]}"#;
        let err = parse_stripe_api_json(json, None).unwrap_err();
        assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
    }
}
