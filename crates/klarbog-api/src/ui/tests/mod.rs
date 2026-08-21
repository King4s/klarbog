//! UI tests: assets, SSR brand, dropdowns, digest round-trip + shared helpers.
//! Flow tests live in `journal_flow` (preview→commit→reversal) and `moms`
//! (3-leg split + settlement).

mod journal_flow;
mod moms;

use super::ui_assets_dir;
use crate::{default_state, router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_types::Actor;
use std::sync::Arc;
use tower::ServiceExt;

/// Router + tempdir-backed company + its UI cookie.
pub(super) async fn test_app() -> (axum::Router, tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &Actor::user("ui-dev"))
        .await
        .unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let cookie = format!(
        "{}={}",
        super::pages::common::COMPANY_COOKIE,
        company_path.to_string_lossy()
    );
    (router(state), dir, cookie)
}

/// POST a form and return the rendered HTML (asserts 200).
pub(super) async fn post_html(app: &axum::Router, cookie: &str, uri: &str, body: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("cookie", cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

/// Flash lines only — for readable assertion messages.
pub(super) fn flashes(html: &str) -> String {
    html.lines()
        .filter(|l| l.contains("flash"))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Pull `value="..."` for a named hidden input and HTML-unescape it.
pub(super) fn extract_input_value(html: &str, name: &str) -> Option<String> {
    let needle = format!("name=\"{name}\" value=\"");
    let start = html.find(&needle)? + needle.len();
    let end = start + html[start..].find('"')?;
    Some(
        html[start..end]
            .replace("&quot;", "\"")
            .replace("&#34;", "\"")
            .replace("&#x27;", "'")
            .replace("&#39;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

pub(super) fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[test]
fn ui_assets_include_styles() {
    let dir = ui_assets_dir();
    assert!(
        dir.join("styles.css").is_file(),
        "expected {}/styles.css",
        dir.display()
    );
}

#[tokio::test]
async fn ui_home_is_rust_ssr() {
    let app = router(default_state());
    let res = app
        .oneshot(Request::builder().uri("/ui/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(html.contains("Klarbog"), "UI should brand Klarbog");
    assert!(
        html.contains("Askama") || html.contains("Rust SSR"),
        "UI should advertise Rust SSR"
    );
    assert!(
        !html.contains("/ui/app.js"),
        "SSR UI must not load the retired JS SPA"
    );
}

/// The journal form must offer the typed chart as dropdowns (ADR-019), with
/// the expense/bank defaults preselected — no free-text account fields.
#[tokio::test]
async fn ui_journal_accounts_are_chart_dropdowns() {
    let app = router(default_state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/ui/journal")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 512 * 1024)
        .await
        .unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(html.contains(r#"<select name="account1""#));
    assert!(html.contains(r#"<select name="account2""#));
    assert!(!html.contains(r#"<input name="account1""#));
    assert!(html.contains("3000 · Software og SaaS"));
    assert!(html.contains("7310 · Forudbetalt indtægt (udskudt omsætning)"));
    // Defaults: expense 3000 debit / bank 2000 credit preselected.
    assert!(html.contains(r#"<option value="3000" selected>"#));
    assert!(html.contains(r#"<option value="2000" selected>"#));
}

/// The invoice commit flow carries the journal entry as JSON through the
/// form; the confirm token is digest-bound, so the serde round-trip must
/// preserve `payload_digest` exactly or commit would fail closed.
#[test]
fn ui_entry_json_roundtrip_preserves_digest() {
    use klarbog_core::payload_digest;
    use klarbog_journal::{Direction, JournalEntry, Leg};
    use klarbog_types::{Currency, MinorAmount};

    let entry = JournalEntry {
        as_of: chrono::Utc::now(),
        memo: "invoice:inv_x:payment æøå".into(),
        actor: Actor::user("ui-dev"),
        legs: vec![
            Leg {
                account: "2000".into(),
                direction: Direction::Debit,
                amount: MinorAmount::from_minor(12_500),
                currency: Currency::new("DKK").unwrap(),
                party_id: None,
            },
            Leg {
                account: "1000".into(),
                direction: Direction::Credit,
                amount: MinorAmount::from_minor(12_500),
                currency: Currency::new("DKK").unwrap(),
                party_id: None,
            },
        ],
    };
    let company = std::path::Path::new("/tmp/co");
    let before = payload_digest(company, &entry).unwrap();
    let json = serde_json::to_string(&entry).unwrap();
    let back: JournalEntry = serde_json::from_str(&json).unwrap();
    let after = payload_digest(company, &back).unwrap();
    assert_eq!(before, after, "entry JSON round-trip must keep the digest");
}
