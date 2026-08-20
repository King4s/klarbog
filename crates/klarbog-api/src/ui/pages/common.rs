//! Shared SSR helpers (ADR-018).

use crate::AppState;
use askama::Template;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};

pub(crate) const COMPANY_COOKIE: &str = "klarbog_ui_company";
pub(crate) const ACTOR: &str = "ui-dev";

pub(crate) struct NavFlags {
    pub home: bool,
    pub parties: bool,
    pub invoices: bool,
    pub bank: bool,
    pub bilag: bool,
    pub journal: bool,
    pub chart: bool,
    pub settings: bool,
}

pub(crate) fn nav(active: &str) -> NavFlags {
    NavFlags {
        home: active == "home",
        parties: active == "parties",
        invoices: active == "invoices",
        bank: active == "bank",
        bilag: active == "bilag",
        journal: active == "journal",
        chart: active == "chart",
        settings: active == "settings",
    }
}

pub(crate) fn company_from(headers: &HeaderMap) -> String {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .into_iter()
        .flat_map(|c| c.split(';'))
        .filter_map(|part| {
            let part = part.trim();
            part.strip_prefix(&format!("{COMPANY_COOKIE}="))
        })
        .map(percent_decode)
        .next()
        .unwrap_or_default()
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

pub(crate) fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn set_company_cookie(company: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{COMPANY_COOKIE}={}; Path=/ui; HttpOnly; SameSite=Lax",
        percent_encode_path(company)
    ))
    .expect("cookie header")
}

pub(crate) fn html_ok(t: impl Template) -> Response {
    match t.render() {
        Ok(body) => Html(body).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("template error: {e}"),
        )
            .into_response(),
    }
}

pub(crate) fn foot(state: &AppState) -> String {
    format!(
        "klarbog-api {} · allowlist {}",
        env!("CARGO_PKG_VERSION"),
        state.allowlist_root.display()
    )
}

pub(crate) async fn authorize_company(
    state: &AppState,
    company: &str,
) -> Result<std::path::PathBuf, String> {
    use klarbog_core::{assert_company_path, open_existing};
    use klarbog_types::Actor;

    if company.is_empty() {
        return Err("Sæt firmasti under Indstillinger.".into());
    }
    let path = assert_company_path(&state.allowlist_root, std::path::Path::new(company))
        .map_err(|e| e.to_string())?;
    let actor = Actor::user(ACTOR);
    open_existing(&path)
        .await
        .map_err(|e| e.to_string())?
        .authorize(&actor)
        .map_err(|e| e.to_string())?;
    Ok(path)
}

pub(crate) fn format_dkk(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let minor = minor.abs();
    format!("{sign}{}.{:02} DKK", minor / 100, minor % 100)
}
