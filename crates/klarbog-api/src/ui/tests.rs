//! UI tests: assets, SSR brand, digest round-trip, journal preview→commit→reversal.

use super::ui_assets_dir;
use crate::{default_state, router};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

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
    use klarbog_types::{Actor, Currency, MinorAmount};

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

/// moms_apply must produce a bookable 3-leg VAT split (net → expense, vat →
/// Købsmoms 4000, gross → credit) through the digest-bound preview→commit.
#[tokio::test]
async fn ui_moms_apply_books_three_leg_vat_split() {
    use crate::AppState;
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_types::Actor;
    use std::sync::Arc;

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
    let app = router(state);
    let cookie = format!(
        "{}={}",
        super::pages::common::COMPANY_COOKIE,
        company_path.to_string_lossy()
    );

    // 12500 øre gross @25% → net 10000, vat 2500.
    let body = "action=moms_apply&moms_gross=12500\
                &moms_memo=kontor%20%23vat25%20%23receipt&account1=3000&account2=2000";
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ui/journal")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let token = extract_input_value(&html, "confirm_token").unwrap_or_else(|| {
        panic!(
            "token in moms_apply preview; flash: {}",
            html.lines()
                .filter(|l| l.contains("flash"))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    });
    let entry_json = extract_input_value(&html, "entry_json").expect("entry_json in preview");
    assert!(entry_json.contains("\"4000\""), "VAT leg on Købsmoms 4000");
    assert!(entry_json.contains("10000"), "net on expense leg");
    assert!(entry_json.contains("2500"), "vat amount");

    let body = format!(
        "action=commit&confirm_token={}&entry_json={}",
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ui/journal")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        html.contains("Commit ok"),
        "moms split commit, got: {}",
        html.lines()
            .filter(|l| l.contains("flash"))
            .collect::<Vec<_>>()
            .join(" | ")
    );
}

/// Regression: the journal UI must commit the *previewed* entry (carried as
/// entry_json), not rebuild it from form fields — rebuilding stamps a fresh
/// `as_of` and the digest-bound token fails closed ("payload mismatch").
#[tokio::test]
async fn ui_journal_preview_then_commit_posts() {
    use crate::AppState;
    use klarbog_core::{default_registry, init_company, ConfirmStore};
    use klarbog_types::Actor;
    use std::sync::Arc;

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
    let app = router(state);
    let cookie = format!(
        "{}={}",
        super::pages::common::COMPANY_COOKIE,
        company_path.to_string_lossy()
    );
    let form = "memo=udgift%20%23vat25%20%23receipt&account1=3000&direction1=debit&amount1=12500\
                &account2=2000&direction2=credit&amount2=12500\
                &moms_gross=12500&moms_memo=x&confirm_token=";

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ui/journal")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("action=preview&{form}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let flash = html
        .lines()
        .filter(|l| l.contains("flash"))
        .collect::<Vec<_>>()
        .join(" | ");
    let token = extract_input_value(&html, "confirm_token")
        .unwrap_or_else(|| panic!("token in preview page; flash: {flash}"));
    let entry_json = extract_input_value(&html, "entry_json").expect("entry_json in preview page");
    assert!(!token.is_empty() && !entry_json.is_empty());

    let body = format!(
        "action=commit&{form}{}&entry_json={}",
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ui/journal")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        html.contains("Commit ok"),
        "commit must post via previewed entry_json, got: {}",
        html.lines()
            .filter(|l| l.contains("flash"))
            .collect::<Vec<_>>()
            .join(" | ")
    );

    // Reversal: preview from the posted id, then commit the exact-negating
    // entry through the same digest-bound panel.
    let posted_id = html
        .split("Commit ok · posted ")
        .nth(1)
        .and_then(|s| s.split('<').next())
        .expect("posted id in commit flash")
        .trim()
        .to_string();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ui/journal")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "action=reverse_preview&entry_id={posted_id}"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let token = extract_input_value(&html, "confirm_token")
        .filter(|t| !t.is_empty())
        .expect("reversal token");
    let entry_json = extract_input_value(&html, "entry_json").expect("reversal entry_json");
    assert!(
        entry_json.contains("tilbagef"),
        "reversal memo should mark the reversal"
    );
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ui/journal")
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "action=commit&confirm_token={}&entry_json={}",
                    urlencoding_encode(&token),
                    urlencoding_encode(&entry_json)
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        html.contains("Commit ok"),
        "reversal commit must post, got: {}",
        html.lines()
            .filter(|l| l.contains("flash"))
            .collect::<Vec<_>>()
            .join(" | ")
    );
}

/// Pull `value="..."` for a named hidden input and HTML-unescape it.
fn extract_input_value(html: &str, name: &str) -> Option<String> {
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

fn urlencoding_encode(s: &str) -> String {
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
