//! Optional HMAC session cookie (ADR-017).

use crate::AppState;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use hmac::{Hmac, Mac};
use klarbog_types::Envelope;
use serde::Deserialize;
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub const COOKIE_NAME: &str = "klarbog_session";
const SESSION_TTL_SECS: i64 = 86_400; // 24h

#[derive(Deserialize)]
pub struct LoginBody {
    pub token: String,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn tokens_equal(presented: &str, expected: &str) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    presented.as_bytes().ct_eq(expected.as_bytes()).into()
}

/// Cookie value: `v1.<exp_unix>.<hex_hmac>` where HMAC covers `v1.<exp_unix>`.
pub fn sign_session(secret: &str, exp_unix: i64) -> String {
    let payload = format!("v1.{exp_unix}");
    let sig = hmac_hex(secret, &payload);
    format!("{payload}.{sig}")
}

pub fn verify_session(secret: &str, cookie: &str, now_unix: i64) -> bool {
    let mut parts = cookie.splitn(3, '.');
    let Some(ver) = parts.next() else {
        return false;
    };
    let Some(exp_s) = parts.next() else {
        return false;
    };
    let Some(sig) = parts.next() else {
        return false;
    };
    if ver != "v1" || sig.is_empty() {
        return false;
    }
    let Ok(exp) = exp_s.parse::<i64>() else {
        return false;
    };
    if exp < now_unix {
        return false;
    }
    let payload = format!("v1.{exp}");
    let expected = hmac_hex(secret, &payload);
    if sig.len() != expected.len() {
        return false;
    }
    bool::from(sig.as_bytes().ct_eq(expected.as_bytes()))
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{name}=");
    for part in raw.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix(&prefix) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn session_cookie_valid(headers: &HeaderMap, secret: &str) -> bool {
    let Some(val) = cookie_value(headers, COOKIE_NAME) else {
        return false;
    };
    verify_session(secret, &val, now_unix())
}

fn build_set_cookie(value: &str, max_age: i64, secure: bool) -> HeaderValue {
    let mut s = format!("{COOKIE_NAME}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}");
    if secure {
        s.push_str("; Secure");
    }
    HeaderValue::from_str(&s).expect("cookie header is ascii")
}

fn clear_set_cookie(secure: bool) -> HeaderValue {
    build_set_cookie("", 0, secure)
}

pub async fn login(State(state): State<AppState>, Json(body): Json<LoginBody>) -> Response {
    let Some(expected) = state.api_token.as_deref().filter(|t| !t.is_empty()) else {
        return (
            StatusCode::NOT_FOUND,
            Json(Envelope::<serde_json::Value>::err([
                "session login not configured (set KLARBOG_API_TOKEN and KLARBOG_SESSION_SECRET)",
            ])),
        )
            .into_response();
    };
    let Some(secret) = state.session_secret.as_deref().filter(|t| !t.is_empty()) else {
        return (
            StatusCode::NOT_FOUND,
            Json(Envelope::<serde_json::Value>::err([
                "session login not configured (set KLARBOG_API_TOKEN and KLARBOG_SESSION_SECRET)",
            ])),
        )
            .into_response();
    };
    let presented = body.token.trim();
    if !tokens_equal(presented, expected) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(Envelope::<serde_json::Value>::err(["invalid login token"])),
        )
            .into_response();
    }
    let exp = now_unix() + SESSION_TTL_SECS;
    let value = sign_session(secret, exp);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        build_set_cookie(&value, SESSION_TTL_SECS, state.session_cookie_secure),
    );
    (
        StatusCode::OK,
        headers,
        Json(Envelope::ok(serde_json::json!({
            "ok": true,
            "expires_at": exp,
        }))),
    )
        .into_response()
}

pub async fn logout(State(state): State<AppState>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        clear_set_cookie(state.session_cookie_secure),
    );
    (
        StatusCode::OK,
        headers,
        Json(Envelope::ok(serde_json::json!({ "ok": true }))),
    )
        .into_response()
}
