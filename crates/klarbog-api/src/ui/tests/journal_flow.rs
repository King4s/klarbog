//! Journal preview→commit→reversal regression.

use super::{extract_input_value, flashes, post_html, test_app, urlencoding_encode};

/// Regression: the journal UI must commit the *previewed* entry (carried as
/// entry_json), not rebuild it from form fields — rebuilding stamps a fresh
/// `as_of` and the digest-bound token fails closed ("payload mismatch").
#[tokio::test]
async fn ui_journal_preview_then_commit_posts() {
    let (app, _dir, cookie) = test_app().await;
    let form = "memo=udgift%20%23vat25%20%23receipt&account1=3000&direction1=debit&amount1=12500\
                &account2=2000&direction2=credit&amount2=12500\
                &moms_gross=12500&moms_memo=x&confirm_token=";

    let html = post_html(
        &app,
        &cookie,
        "/ui/journal",
        &format!("action=preview&{form}"),
    )
    .await;
    let token = extract_input_value(&html, "confirm_token")
        .unwrap_or_else(|| panic!("token in preview page; flash: {}", flashes(&html)));
    let entry_json = extract_input_value(&html, "entry_json").expect("entry_json in preview page");
    assert!(!token.is_empty() && !entry_json.is_empty());

    let body = format!(
        "action=commit&{form}{}&entry_json={}",
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    let html = post_html(&app, &cookie, "/ui/journal", &body).await;
    assert!(
        html.contains("Commit ok"),
        "commit must post via previewed entry_json, got: {}",
        flashes(&html)
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
    let html = post_html(
        &app,
        &cookie,
        "/ui/journal",
        &format!("action=reverse_preview&entry_id={posted_id}"),
    )
    .await;
    let token = extract_input_value(&html, "confirm_token")
        .filter(|t| !t.is_empty())
        .expect("reversal token");
    let entry_json = extract_input_value(&html, "entry_json").expect("reversal entry_json");
    assert!(
        entry_json.contains("tilbagef"),
        "reversal memo should mark the reversal"
    );
    let body = format!(
        "action=commit&confirm_token={}&entry_json={}",
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    let html = post_html(&app, &cookie, "/ui/journal", &body).await;
    assert!(
        html.contains("Commit ok"),
        "reversal commit must post, got: {}",
        flashes(&html)
    );
}
