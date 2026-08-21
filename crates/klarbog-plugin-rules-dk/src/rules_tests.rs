use super::*;
use chrono::Utc;
use klarbog_journal::{Direction, Leg};
use klarbog_types::{Actor, Currency, MinorAmount, PartyId};

fn entry(memo: &str, minor: i64) -> JournalEntry {
    entry_with_party(memo, minor, None)
}

fn entry_with_party(memo: &str, minor: i64, party_id: Option<PartyId>) -> JournalEntry {
    let amount = MinorAmount::from_minor(minor);
    let currency = Currency::new("DKK").unwrap();
    JournalEntry {
        as_of: Utc::now(),
        memo: memo.into(),
        actor: Actor::user("t"),
        legs: vec![
            Leg {
                account: "3000".into(),
                direction: Direction::Debit,
                amount,
                currency: currency.clone(),
                party_id: party_id.clone(),
            },
            Leg {
                account: "2000".into(),
                direction: Direction::Credit,
                amount,
                currency,
                party_id: None,
            },
        ],
    }
}

#[test]
fn rejects_empty_memo() {
    let err = RULES_DK.validate_entry(&entry("", 100)).unwrap_err();
    assert!(err.to_string().contains("memo"));
}

#[test]
fn rejects_zero_amount_leg() {
    let err = RULES_DK.validate_entry(&entry("ok", 0)).unwrap_err();
    assert!(err.to_string().contains("zero amount"));
}

#[test]
fn rejects_invalid_account() {
    let mut e = entry("ok #receipt", 100);
    e.legs[0].account = "6abc".into();
    let err = RULES_DK.validate_entry(&e).unwrap_err();
    assert!(err.to_string().contains("account_range"));
}

#[test]
fn rejects_empty_account() {
    let mut e = entry("ok #receipt", 100);
    e.legs[0].account = "   ".into();
    let err = RULES_DK.validate_entry(&e).unwrap_err();
    assert!(err.to_string().contains("account_range"));
}

#[test]
fn rejects_unsupported_vat_rate() {
    let err = RULES_DK
        .validate_entry(&entry("office vat:12 #receipt", 100))
        .unwrap_err();
    assert!(err.to_string().contains("dk.vat.rate"));
}

#[test]
fn flags_vat_rate_hint() {
    let applied = RULES_DK
        .validate_entry(&entry("supplies moms:25 #receipt", 500))
        .unwrap();
    assert!(applied.contains(&"dk.vat.rate".to_string()));
    assert!(applied.contains(&"dk.vat.split_hint".to_string()));
}

#[test]
fn vat0_rate_without_split_hint() {
    let applied = RULES_DK
        .validate_entry(&entry("zero-rated #vat0 #receipt", 500))
        .unwrap();
    assert!(applied.contains(&"dk.vat.rate".to_string()));
    assert!(!applied.contains(&"dk.vat.split_hint".to_string()));
}

#[test]
fn blocks_expense_without_party_or_receipt() {
    let err = RULES_DK
        .validate_entry(&entry("office supplies", 500))
        .unwrap_err();
    assert!(err.to_string().contains("dk.expense.receipt_required"));
}

#[test]
fn allows_expense_with_receipt_tag() {
    let applied = RULES_DK
        .validate_entry(&entry("office supplies #receipt", 500))
        .unwrap();
    assert!(!applied.contains(&"dk.expense.receipt_hint".to_string()));
}

#[test]
fn allows_expense_with_document_id_signal() {
    let applied = RULES_DK
        .validate_entry(&entry("office document_id:doc-9", 500))
        .unwrap();
    assert!(applied.is_empty());
}

#[test]
fn hints_when_party_without_receipt() {
    let party = PartyId::new("vendor-1");
    let applied = RULES_DK
        .validate_entry(&entry_with_party("office supplies", 500, Some(party)))
        .unwrap();
    assert_eq!(applied, vec!["dk.expense.receipt_hint"]);
}

#[test]
fn allows_party_plus_receipt_tag() {
    let party = PartyId::new("vendor-1");
    let applied = RULES_DK
        .validate_entry(&entry_with_party("office #receipt", 500, Some(party)))
        .unwrap();
    assert!(!applied.contains(&"dk.expense.receipt_hint".to_string()));
}

#[test]
fn no_receipt_rule_without_expense_debit() {
    // Bank debit (money in) against revenue credit — receipt rule must not fire.
    let amount = MinorAmount::from_minor(100);
    let currency = Currency::new("DKK").unwrap();
    let e = JournalEntry {
        as_of: Utc::now(),
        memo: "payment received".into(),
        actor: Actor::user("t"),
        legs: vec![
            Leg {
                account: "2000".into(),
                direction: Direction::Debit,
                amount,
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: "1000".into(),
                direction: Direction::Credit,
                amount,
                currency,
                party_id: None,
            },
        ],
    };
    let applied = RULES_DK.validate_entry(&e).unwrap();
    assert!(applied.is_empty());
}

#[test]
fn staff_and_depreciation_debits_exempt_from_receipt_rule() {
    for account in ["3500", "5820"] {
        let mut e = entry("løn/afskrivning uden bilag", 500);
        e.legs[0].account = account.into();
        let applied = RULES_DK.validate_entry(&e).unwrap();
        assert!(
            !applied.iter().any(|r| r.contains("receipt")),
            "{account}: {applied:?}"
        );
    }
}

#[test]
fn hints_when_account_outside_chart_stub() {
    let amount = MinorAmount::from_minor(100);
    let currency = Currency::new("DKK").unwrap();
    let e = JournalEntry {
        as_of: Utc::now(),
        memo: "misc".into(),
        actor: Actor::user("t"),
        legs: vec![
            Leg {
                // Old stub codes are outside the chart now — hint fires.
                account: "6000".into(),
                direction: Direction::Debit,
                amount,
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: "1000".into(),
                direction: Direction::Credit,
                amount,
                currency,
                party_id: None,
            },
        ],
    };
    let applied = RULES_DK.validate_entry(&e).unwrap();
    assert_eq!(applied, vec![RULE_KNOWN_ACCOUNT]);
}

#[test]
fn known_stub_accounts_skip_known_account_hint() {
    let amount = MinorAmount::from_minor(100);
    let currency = Currency::new("DKK").unwrap();
    let e = JournalEntry {
        as_of: Utc::now(),
        memo: "ar settle".into(),
        actor: Actor::user("t"),
        legs: vec![
            Leg {
                account: "2000".into(),
                direction: Direction::Debit,
                amount,
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: "1100".into(),
                direction: Direction::Credit,
                amount,
                currency,
                party_id: None,
            },
        ],
    };
    let applied = RULES_DK.validate_entry(&e).unwrap();
    assert!(!applied.iter().any(|r| r == RULE_KNOWN_ACCOUNT));
}

#[test]
fn expense_and_creditors_are_known() {
    let amount = MinorAmount::from_minor(250);
    let currency = Currency::new("DKK").unwrap();
    let e = JournalEntry {
        as_of: Utc::now(),
        memo: "bill #receipt".into(),
        actor: Actor::user("t"),
        legs: vec![
            Leg {
                account: "3000".into(),
                direction: Direction::Debit,
                amount,
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: "7000".into(),
                direction: Direction::Credit,
                amount,
                currency,
                party_id: None,
            },
        ],
    };
    let applied = RULES_DK.validate_entry(&e).unwrap();
    assert!(!applied.iter().any(|r| r == RULE_KNOWN_ACCOUNT));
}
