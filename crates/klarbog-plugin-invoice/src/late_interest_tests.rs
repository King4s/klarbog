//! Tests for late_interest (kept separate for linegate).

use super::late_interest::*;
use crate::due_date::STATUTORY_PAYMENT_TERM_DAYS;
use crate::store::{create_draft_from_new, NewLine};
use crate::{InvoiceError, InvoiceKind, InvoicePayment, InvoiceStatus};
use klarbog_plugin_crm::upsert_party;
use std::path::Path;
use tempfile::tempdir;

fn issued_invoice(co: &Path, gross_minor: i64, issue: &str, due: &str) -> crate::Invoice {
    let party = upsert_party(
        co,
        None,
        "Buyer".into(),
        klarbog_plugin_crm::PartyKind::Private,
    )
    .unwrap();
    let inv = create_draft_from_new(
        co,
        party.id,
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Work".into(),
            amount_minor: gross_minor,
            currency: "DKK".into(),
        }],
    )
    .unwrap();
    crate::issue::record_issue(
        co,
        &inv.id,
        issue.into(),
        STATUTORY_PAYMENT_TERM_DAYS as u32,
        Some("2026-0001".into()),
        None,
        None,
    )
    .unwrap();
    let mut file = crate::store::load(co).unwrap();
    let stored = file.invoices.iter_mut().find(|i| i.id == inv.id).unwrap();
    stored.due_date = Some(due.into());
    stored.status = InvoiceStatus::Sent;
    crate::store::save(co, &file).unwrap();
    crate::store::load(co)
        .unwrap()
        .invoices
        .into_iter()
        .find(|i| i.id == inv.id)
        .unwrap()
}

fn date_ms(date: &str) -> i64 {
    crate::due_date::parse_iso_date(date)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

#[test]
fn statutory_table_lookup_2026() {
    use crate::due_date::parse_iso_date;
    assert_eq!(
        lookup_statutory_reference_rate(parse_iso_date("2026-02-01").unwrap()),
        Some(175)
    );
    assert_eq!(175 + STATUTORY_SURCHARGE_BPS, 975);
}

#[test]
fn cumulative_interest_matches_reference_case() {
    let segments = vec![InterestSegment {
        principal_minor: 25_000,
        annual_rate_bps: 1020,
        days: 5,
    }];
    assert_eq!(cumulative_interest_minor(&segments).unwrap(), 35);
}

#[test]
fn calculates_overdue_partial_payment() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let mut inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    inv.payments.push(InvoicePayment {
        unix_ms: date_ms("2026-05-20"),
        amount_minor: 100_000,
        currency: inv.lines[0].currency.clone(),
    });
    let calc =
        calculate_late_interest(&inv, parse_iso_date("2026-06-20").unwrap(), Some(220)).unwrap();
    assert_eq!(calc.overdue_days, 5);
    assert_eq!(calc.principal_open_minor, 25_000);
    assert_eq!(calc.annual_interest_rate_bps, 1020);
    assert_eq!(calc.accrued_interest_minor, 35);
}

#[test]
fn register_rejects_duplicate_and_zero_increment() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    let as_of = parse_iso_date("2026-06-20").unwrap();
    register_late_interest(&co, &inv.id, as_of, Some(220), None).unwrap();
    let dup = register_late_interest(&co, &inv.id, as_of, Some(220), None);
    assert!(matches!(
        dup,
        Err(InvoiceError::DuplicateInterestClaim { .. })
    ));
}

#[test]
fn staged_claims_bill_incrementally() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    register_late_interest(
        &co,
        &inv.id,
        parse_iso_date("2026-06-20").unwrap(),
        Some(220),
        None,
    )
    .unwrap();
    let second = register_late_interest(
        &co,
        &inv.id,
        parse_iso_date("2026-07-01").unwrap(),
        Some(220),
        None,
    )
    .unwrap();
    assert!(second.1.accrued_interest_minor > 0);
    assert!(second.1.prior_claimed_interest_minor >= 35);
}

#[test]
fn defaults_to_statutory_table() {
    use crate::due_date::parse_iso_date;
    let dir = tempdir().unwrap();
    let co = dir.path().join("co");
    std::fs::create_dir_all(&co).unwrap();
    let inv = issued_invoice(&co, 125_000, "2026-05-16", "2026-06-15");
    let calc = calculate_late_interest(&inv, parse_iso_date("2026-02-01").unwrap(), None).unwrap();
    assert_eq!(calc.reference_rate_bps, 175);
    assert_eq!(calc.annual_interest_rate_bps, 975);
    assert_eq!(
        calc.reference_rate_source,
        ReferenceRateSource::StatutoryTable
    );
}
