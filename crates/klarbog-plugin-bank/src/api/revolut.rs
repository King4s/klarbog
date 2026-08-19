//! Revolut Business API transaction fetch + JSON parse (ADR-008).

use crate::amount::parse_amount_minor;
use crate::api::{BankApiError, HttpClient, ReqwestHttpClient};
use crate::config::RevolutApiConfig;
use crate::csv::{validate_currencies, BankCsvError, BankRow};
use chrono::{DateTime, Utc};
use klarbog_types::Currency;
use serde_json::Value;

pub async fn fetch_revolut_transactions(
    cfg: &RevolutApiConfig,
    since_unix: Option<i64>,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankApiError> {
    fetch_revolut_transactions_with_client(&ReqwestHttpClient, cfg, since_unix, required_currency)
        .await
}

pub async fn fetch_revolut_transactions_with_client<C: HttpClient>(
    client: &C,
    cfg: &RevolutApiConfig,
    since_unix: Option<i64>,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankApiError> {
    let mut url = format!("{}/transactions", cfg.base_url.trim_end_matches('/'));
    if let Some(ts) = since_unix {
        let dt = DateTime::from_timestamp(ts, 0)
            .ok_or_else(|| BankApiError::Parse(format!("invalid since_unix: {ts}")))?;
        url.push_str(&format!("?from={}", dt.to_rfc3339()));
    }
    let (status, body) = client.get(&url, &cfg.token).await?;
    if status != 200 {
        return Err(BankApiError::Http { status, body });
    }
    Ok(parse_revolut_api_json(&body, required_currency)?)
}

pub fn parse_revolut_api_json(
    json: &str,
    required_currency: Option<&Currency>,
) -> Result<Vec<BankRow>, BankCsvError> {
    let root: Value = serde_json::from_str(json).map_err(|e| BankCsvError::Row {
        row: 0,
        detail: format!("invalid json: {e}"),
    })?;
    let items = root
        .as_array()
        .or_else(|| root.get("data").and_then(|v| v.as_array()))
        .ok_or(BankCsvError::Row {
            row: 0,
            detail: "expected array of transactions".into(),
        })?;

    let mut out = Vec::new();
    let mut currencies = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let row_no = i + 1;
        if item
            .get("state")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("declined"))
        {
            continue;
        }
        let date_raw = item
            .get("completed_at")
            .or_else(|| item.get("created_at"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| BankCsvError::Row {
                row: row_no,
                detail: "missing completed_at/created_at".into(),
            })?;
        let date = DateTime::parse_from_rfc3339(date_raw)
            .map_err(|e| BankCsvError::Row {
                row: row_no,
                detail: format!("invalid date: {e}"),
            })?
            .with_timezone(&Utc);

        let text = item
            .get("description")
            .or_else(|| item.get("reference"))
            .and_then(|v| v.as_str())
            .unwrap_or("Revolut transaction")
            .to_string();

        let (amount_raw, currency_raw) = revolut_amount_currency(item, row_no)?;
        let code = currency_raw.trim().to_uppercase();
        Currency::new(&code).map_err(|_| BankCsvError::Row {
            row: row_no,
            detail: format!("invalid currency: {currency_raw}"),
        })?;
        currencies.push(code);

        let amount_minor = parse_amount_minor(&amount_raw).map_err(|e| BankCsvError::Row {
            row: row_no,
            detail: e.to_string(),
        })?;
        out.push(BankRow {
            date,
            text,
            amount_minor,
        });
    }

    validate_currencies(&currencies, required_currency)?;
    Ok(out)
}

fn revolut_amount_currency(item: &Value, row_no: usize) -> Result<(String, String), BankCsvError> {
    if let Some(legs) = item.get("legs").and_then(|v| v.as_array()) {
        if let Some(leg) = legs.first() {
            let amount =
                json_scalar_to_string(leg.get("amount").ok_or_else(|| BankCsvError::Row {
                    row: row_no,
                    detail: "leg missing amount".into(),
                })?)?;
            let currency = leg
                .get("currency")
                .and_then(|v| v.as_str())
                .ok_or_else(|| BankCsvError::Row {
                    row: row_no,
                    detail: "leg missing currency".into(),
                })?;
            return Ok((amount, currency.to_string()));
        }
    }
    let amount = json_scalar_to_string(item.get("amount").ok_or_else(|| BankCsvError::Row {
        row: row_no,
        detail: "missing amount".into(),
    })?)?;
    let currency = item
        .get("currency")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BankCsvError::Row {
            row: row_no,
            detail: "missing currency".into(),
        })?;
    Ok((amount, currency.to_string()))
}

fn json_scalar_to_string(val: &Value) -> Result<String, BankCsvError> {
    match val {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(BankCsvError::Row {
            row: 0,
            detail: "amount must be string or number".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::HttpClient;

    const FIXTURE: &str = include_str!("../../tests/fixtures/revolut_api.json");

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
    }

    #[test]
    fn parse_fixture_matches_csv_amounts() {
        let rows = parse_revolut_api_json(FIXTURE, Some(&Currency::new("DKK").unwrap())).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].amount_minor.minor(), -4550);
        assert_eq!(rows[0].text, "Coffee shop");
        assert_eq!(rows[2].text, "Transfer, internal");
    }

    #[tokio::test]
    async fn fetch_with_mock_client() {
        let cfg = RevolutApiConfig {
            token: "test-token".into(),
            base_url: "https://example.test/api/1.0".into(),
        };
        let client = MockClient {
            body: FIXTURE.to_string(),
        };
        let rows = fetch_revolut_transactions_with_client(
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
        let json = r#"[
          {"completed_at":"2026-01-01T00:00:00Z","description":"A","legs":[{"amount":"-10","currency":"DKK"}]},
          {"completed_at":"2026-01-02T00:00:00Z","description":"B","legs":[{"amount":"5","currency":"EUR"}]}
        ]"#;
        let err = parse_revolut_api_json(json, None).unwrap_err();
        assert!(matches!(err, BankCsvError::MixedCurrency { .. }));
    }
}
