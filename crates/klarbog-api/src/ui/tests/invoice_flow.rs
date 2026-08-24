//! Invoice morarente UI flow (DK-INVOICE-LATE-INTEREST-BOOKKEEPING-001).

use super::{extract_input_value, flashes, post_html, test_app, urlencoding_encode};
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_invoice::{
    create_draft_from_new, due_date::STATUTORY_PAYMENT_TERM_DAYS, get_invoice, record_issue,
    InvoiceKind, InvoiceStatus, NewLine,
};

#[tokio::test]
async fn ui_interest_register_and_post() {
    let (app, dir, cookie) = test_app().await;
    let co = dir.path().join("co");
    let party = upsert_party(
        &co,
        None,
        "Interest buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Overdue work".into(),
            amount_minor: 125_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    record_issue(
        &co,
        &inv.id,
        "2026-05-16".into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some("2026-0099".into()),
        None,
        None,
    )
    .unwrap();
    let stored = get_invoice(&co, &inv.id).unwrap().unwrap();
    assert_eq!(stored.status, InvoiceStatus::Sent);

    let html = post_html(
        &app,
        &cookie,
        "/ui/invoices",
        &format!(
            "action=interest_register&invoice_id={}&as_of_date=2026-06-20&reference_rate_bps=220",
            inv.id
        ),
    )
    .await;
    assert!(
        html.contains("Morarente registreret"),
        "register flash missing: {}",
        flashes(&html)
    );

    let html = post_html(
        &app,
        &cookie,
        "/ui/invoices",
        &format!("action=interest_post_preview&invoice_id={}", inv.id),
    )
    .await;
    let token = extract_input_value(&html, "confirm_token")
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| panic!("interest preview token; {}", flashes(&html)));
    let entry_json = extract_input_value(&html, "entry_json").expect("interest entry_json");
    assert!(
        entry_json.contains(":interest:2026-06-20"),
        "interest memo should carry claim date"
    );
    assert!(entry_json.contains("1010"), "interest income leg 1010");

    let body = format!(
        "action=commit_interest&invoice_id={}&confirm_token={}&entry_json={}",
        inv.id,
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    let html = post_html(&app, &cookie, "/ui/invoices", &body).await;
    assert!(
        html.contains("Morarente bogført"),
        "interest commit failed: {}",
        flashes(&html)
    );

    let posted = get_invoice(&co, &inv.id).unwrap().unwrap();
    assert_eq!(posted.interest_claims.len(), 1);
    assert!(posted.interest_claims[0].posted_journal_id.is_some());
}

#[tokio::test]
async fn ui_reminder_register_and_post() {
    let (app, dir, cookie) = test_app().await;
    let co = dir.path().join("co");
    let party = upsert_party(
        &co,
        None,
        "Reminder buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        None,
    )
    .unwrap();
    let inv = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Overdue work".into(),
            amount_minor: 125_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    record_issue(
        &co,
        &inv.id,
        "2026-05-16".into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some("2026-0100".into()),
        None,
        None,
    )
    .unwrap();
    let stored = get_invoice(&co, &inv.id).unwrap().unwrap();
    assert_eq!(stored.status, InvoiceStatus::Sent);

    let html = post_html(
        &app,
        &cookie,
        "/ui/invoices",
        &format!(
            "action=reminder_register&invoice_id={}&reminder_date=2026-06-26",
            inv.id
        ),
    )
    .await;
    assert!(
        html.contains("Rykker registreret"),
        "register flash missing: {}",
        flashes(&html)
    );

    let html = post_html(
        &app,
        &cookie,
        "/ui/invoices",
        &format!("action=reminder_post_preview&invoice_id={}", inv.id),
    )
    .await;
    let token = extract_input_value(&html, "confirm_token")
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| panic!("reminder preview token; {}", flashes(&html)));
    let entry_json = extract_input_value(&html, "entry_json").expect("reminder entry_json");
    assert!(
        entry_json.contains(":reminder:2026-06-26"),
        "reminder memo should carry reminder date"
    );
    assert!(entry_json.contains("1010"), "reminder income leg 1010");

    let body = format!(
        "action=commit_reminder&invoice_id={}&confirm_token={}&entry_json={}",
        inv.id,
        urlencoding_encode(&token),
        urlencoding_encode(&entry_json)
    );
    let html = post_html(&app, &cookie, "/ui/invoices", &body).await;
    assert!(
        html.contains("Rykkergebyr bogført"),
        "reminder commit failed: {}",
        flashes(&html)
    );

    let posted = get_invoice(&co, &inv.id).unwrap().unwrap();
    assert_eq!(posted.reminders.len(), 1);
    assert!(posted.reminders[0].posted_journal_id.is_some());
}

#[tokio::test]
async fn ui_send_reminder_compound() {
    use klarbog_plugin_invoice::read_send_log;
    use std::fs;

    let (app, dir, cookie) = test_app().await;
    let co = dir.path().join("co");
    let party = upsert_party(
        &co,
        None,
        "Reminder email buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
        None,
        Some("buyer@example.com".into()),
    )
    .unwrap();
    let inv = create_draft_from_new(
        &co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Overdue work".into(),
            amount_minor: 125_000,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    let invoice_no = "2026-0101";
    let object_dir = co.join("objects/invoices/issued");
    fs::create_dir_all(&object_dir).unwrap();
    fs::write(
        object_dir.join(format!("{invoice_no}.json")),
        br#"{"type":"issued_invoice","invoiceNumber":"2026-0101"}"#,
    )
    .unwrap();
    record_issue(
        &co,
        &inv.id,
        "2026-05-16".into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some(invoice_no.into()),
        Some("doc_reminder_send".into()),
        None,
    )
    .unwrap();

    let html = post_html(
        &app,
        &cookie,
        "/ui/invoices",
        &format!(
            "action=send_reminder&invoice_id={}&reminder_date=2026-06-26&email_to=buyer%40example.com",
            inv.id
        ),
    )
    .await;
    assert!(
        html.contains("Rykker sendt"),
        "compound send failed: {}",
        flashes(&html)
    );
    assert!(
        html.contains("gebyr bogført"),
        "expected inline fee booking: {}",
        flashes(&html)
    );

    let posted = get_invoice(&co, &inv.id).unwrap().unwrap();
    assert_eq!(posted.reminders.len(), 1);
    assert_eq!(posted.reminders[0].reminder_date, "2026-06-26");
    assert!(posted.reminders[0].posted_journal_id.is_some());

    let log = read_send_log(&co).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].kind, klarbog_plugin_invoice::EmailKind::Reminder);
    assert_eq!(log[0].recipient, "buyer@example.com");
}
