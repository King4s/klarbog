//! Moms flows: 3-leg VAT split (journal) and settlement (chart), both through
//! the digest-bound preview→commit two-phase.

use super::{extract_input_value, flashes, post_html, test_app, urlencoding_encode};

const SPLIT_FORM: &str = "action=moms_apply&moms_gross=12500\
                          &moms_memo=kontor%20%23vat25%20%23receipt&account1=3000&account2=2000";

/// Commit a 12500-øre gross split (net 10000 → 3000, vat 2500 → 4000) and
/// return the commit page HTML.
async fn commit_moms_split(app: &axum::Router, cookie: &str) -> String {
    let html = post_html(app, cookie, "/ui/journal", SPLIT_FORM).await;
    let token = extract_input_value(&html, "confirm_token")
        .unwrap_or_else(|| panic!("split preview token; flash: {}", flashes(&html)));
    let entry_json = extract_input_value(&html, "entry_json").expect("split entry_json");
    assert!(entry_json.contains("\"4000\""), "VAT leg on Købsmoms 4000");
    assert!(entry_json.contains("10000"), "net on expense leg");
    assert!(entry_json.contains("2500"), "vat amount");
    let body = format!(
        "action=commit&confirm_token={}&entry_json={}",
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    post_html(app, cookie, "/ui/journal", &body).await
}

/// moms_apply must produce a bookable 3-leg VAT split (net → expense, vat →
/// Købsmoms 4000, gross → credit) through the digest-bound preview→commit.
#[tokio::test]
async fn ui_moms_apply_books_three_leg_vat_split() {
    let (app, _dir, cookie) = test_app().await;
    let html = commit_moms_split(&app, &cookie).await;
    assert!(
        html.contains("Commit ok"),
        "moms split commit, got: {}",
        flashes(&html)
    );
}

/// Momsafregning: with only købsmoms on the books, settle_preview must build
/// a receivable (4500 debit) entry and commit must zero Købsmoms 4000.
#[tokio::test]
async fn ui_chart_moms_settlement_two_phase() {
    let (app, _dir, cookie) = test_app().await;
    let html = commit_moms_split(&app, &cookie).await;
    assert!(html.contains("Commit ok"), "seed commit failed");

    // Settlement preview: no salgsmoms → net is a receivable (4500 debit).
    let html = post_html(&app, &cookie, "/ui/chart", "action=settle_preview").await;
    let token = extract_input_value(&html, "confirm_token")
        .unwrap_or_else(|| panic!("settle preview token; flash: {}", flashes(&html)));
    let entry_json = extract_input_value(&html, "entry_json").expect("settle entry_json");
    assert!(entry_json.contains("\"4500\""), "settlement leg on 4500");
    assert!(entry_json.contains("momsafregning"), "settlement memo");
    assert!(html.contains("Momstilgodehavende"), "receivable label");

    let body = format!(
        "action=settle_commit&confirm_token={}&entry_json={}",
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    let html = post_html(&app, &cookie, "/ui/chart", &body).await;
    assert!(
        html.contains("Momsafregning bogført"),
        "settle commit, got: {}",
        flashes(&html)
    );
    // Købsmoms is zeroed → nothing left to settle.
    assert!(html.contains("Ingen moms at afregne"), "vat zeroed");
    let html = post_html(&app, &cookie, "/ui/chart", "action=settle_preview").await;
    assert!(
        html.contains("Ingen moms at afregne"),
        "second settle must fail closed"
    );
}
