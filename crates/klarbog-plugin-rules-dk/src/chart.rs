//! Klarbog chart of accounts — mirrors the original project's `seedAccounts`
//! (ADR-019 Phase 1). Semantics derive from per-account **type**, not numeric
//! bands. Digits outside the chart still validate as account strings; rules-dk
//! only emits the optional hint [`RULE_KNOWN_ACCOUNT`].

use serde::Serialize;

/// Applied-rule id when a digit account is outside the chart (hint only).
pub const RULE_KNOWN_ACCOUNT: &str = "dk.bookkeeping.known_account";

/// Staff-cost accounts (Lønninger m.fl.) — expense type but never
/// receipt-gated: salaries have no receipts.
pub const DK_CHART_STAFF_MIN: i64 = 3_500;
/// Inclusive end of the staff-cost range.
pub const DK_CHART_STAFF_MAX: i64 = 3_599;
/// Afskrivninger — expense type but a non-cash internal booking, exempt from
/// the receipt rule like staff costs.
pub const DK_CHART_DEPRECIATION: i64 = 5_820;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Income,
    Expense,
    Asset,
    Liability,
    Equity,
    Vat,
}

/// One chart row (code, Danish label, type, normal balance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChartAccount {
    pub code: i64,
    pub label: &'static str,
    #[serde(rename = "type")]
    pub account_type: AccountType,
    /// `true` when the account is credit-normal.
    pub credit_normal: bool,
}

const fn acc(code: i64, label: &'static str, t: AccountType, credit_normal: bool) -> ChartAccount {
    ChartAccount {
        code,
        label,
        account_type: t,
        credit_normal,
    }
}

use AccountType as T;

/// The chart, mirroring the original project's seed (48 accounts).
pub const DK_CHART: &[ChartAccount] = &[
    acc(1000, "Omsætning, ydelser", T::Income, true),
    acc(1010, "Gebyr- og kompensationsindtægter", T::Income, true),
    acc(1020, "Valutakursgevinst (realiseret)", T::Income, true),
    acc(1100, "Debitorer", T::Asset, false),
    acc(1200, "Salgsmoms", T::Vat, true),
    acc(1300, "Forudbetalte omkostninger", T::Asset, false),
    acc(2000, "Bank", T::Asset, false),
    acc(3000, "Software og SaaS", T::Expense, false),
    acc(3010, "AI-værktøjer", T::Expense, false),
    acc(3020, "Hosting og cloud", T::Expense, false),
    acc(3050, "Rejse og transport", T::Expense, false),
    acc(3055, "Kørselsgodtgørelse", T::Expense, false),
    acc(3070, "Repræsentation", T::Expense, false),
    acc(3080, "Tab på debitorer", T::Expense, false),
    acc(3100, "Husleje", T::Expense, false),
    acc(3110, "El, vand og varme", T::Expense, false),
    acc(3120, "Hardware og udstyr", T::Expense, false),
    acc(3130, "Kontorartikler og småanskaffelser", T::Expense, false),
    acc(3140, "Telefon og internet", T::Expense, false),
    acc(3150, "Forsikringer", T::Expense, false),
    acc(3160, "Revisor og bogføring", T::Expense, false),
    acc(3170, "Advokat og rådgivning", T::Expense, false),
    acc(3180, "Markedsføring og annoncering", T::Expense, false),
    acc(3190, "Kontingenter og abonnementer", T::Expense, false),
    acc(3200, "Porto og fragt", T::Expense, false),
    acc(3300, "Gebyrer, bank og betalingskort", T::Expense, false),
    acc(3310, "Renteudgifter", T::Expense, false),
    acc(3320, "Valutakurstab (realiseret)", T::Expense, false),
    acc(3500, "Lønninger", T::Expense, false),
    acc(3510, "Pension", T::Expense, false),
    acc(3520, "ATP og lovpligtige bidrag", T::Expense, false),
    acc(3530, "Personaleomkostninger", T::Expense, false),
    acc(4000, "Købsmoms", T::Vat, false),
    acc(4500, "Momsafregning", T::Liability, true),
    acc(5000, "Egenkapital", T::Equity, true),
    acc(5010, "Privat hævning", T::Equity, false),
    acc(5020, "Privat indskud", T::Equity, true),
    acc(5800, "Driftsmidler og inventar", T::Asset, false),
    acc(5810, "Akkumulerede afskrivninger", T::Asset, true),
    acc(5820, "Afskrivninger", T::Expense, false),
    acc(7000, "Leverandørgæld (kreditorer)", T::Liability, true),
    acc(7100, "Skyldig A-skat", T::Liability, true),
    acc(7110, "Skyldigt AM-bidrag", T::Liability, true),
    acc(7120, "Skyldig ATP", T::Liability, true),
    acc(7130, "Skyldig løn", T::Liability, true),
    acc(7200, "Skyldig skat (skattekonto)", T::Liability, true),
    acc(7300, "Skyldige omkostninger", T::Liability, true),
    acc(
        7310,
        "Forudbetalt indtægt (udskudt omsætning)",
        T::Liability,
        true,
    ),
];

/// Chart row lookup by numeric code.
pub fn find_account(code: i64) -> Option<&'static ChartAccount> {
    DK_CHART.iter().find(|a| a.code == code)
}

/// Full chart for HTTP/MCP/SSR read surfaces.
pub fn chart_accounts() -> &'static [ChartAccount] {
    DK_CHART
}

/// True when the code is an expense-type account in the chart.
#[inline]
pub fn is_expense_account_code(n: i64) -> bool {
    matches!(find_account(n), Some(a) if a.account_type == AccountType::Expense)
}

/// True when an expense debit on this code must carry a receipt/party signal.
/// Staff costs and depreciation are expense-type but exempt (no receipts).
#[inline]
pub fn is_receipt_gated_expense_code(n: i64) -> bool {
    if (DK_CHART_STAFF_MIN..=DK_CHART_STAFF_MAX).contains(&n) || n == DK_CHART_DEPRECIATION {
        return false;
    }
    is_expense_account_code(n)
}

/// True when the account string parses as i64 and exists in the chart.
pub fn is_known_dk_account(account: &str) -> bool {
    account.parse::<i64>().ok().and_then(find_account).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_codes_are_known_and_typed() {
        for (code, t) in [
            (1000, AccountType::Income),
            (1100, AccountType::Asset),
            (1200, AccountType::Vat),
            (2000, AccountType::Asset),
            (3000, AccountType::Expense),
            (4000, AccountType::Vat),
            (4500, AccountType::Liability),
            (5000, AccountType::Equity),
            (7000, AccountType::Liability),
        ] {
            let a = find_account(code).unwrap_or_else(|| panic!("{code} missing"));
            assert_eq!(a.account_type, t, "{code}");
            assert!(is_known_dk_account(&code.to_string()));
        }
        assert_eq!(DK_CHART.len(), 48);
    }

    #[test]
    fn old_stub_codes_are_no_longer_known() {
        for code in ["1500", "4400", "6000", "6100", "6999"] {
            assert!(!is_known_dk_account(code), "{code}");
        }
    }

    #[test]
    fn expense_semantics_from_type_not_bands() {
        // Bank (2000) and creditors (7000) are not expenses.
        assert!(!is_expense_account_code(2000));
        assert!(!is_expense_account_code(7000));
        // 3xxx operating costs are receipt-gated expenses.
        assert!(is_expense_account_code(3000));
        assert!(is_receipt_gated_expense_code(3000));
        // Staff and depreciation: expense type, but exempt from the gate.
        for code in [3500, 3510, 3520, 3530, 5820] {
            assert!(is_expense_account_code(code), "{code}");
            assert!(!is_receipt_gated_expense_code(code), "{code}");
        }
        // Old stub band members are not expenses anymore.
        assert!(!is_expense_account_code(6000));
        assert!(!is_expense_account_code(5800));
    }

    #[test]
    fn non_digit_not_known() {
        assert!(!is_known_dk_account("6abc"));
        assert!(!is_known_dk_account(""));
        assert!(!is_known_dk_account("30_00"));
    }

    #[test]
    fn credit_normal_matches_original_seed() {
        assert!(find_account(1000).unwrap().credit_normal);
        assert!(!find_account(2000).unwrap().credit_normal);
        assert!(find_account(5810).unwrap().credit_normal, "contra-asset");
        assert!(!find_account(4000).unwrap().credit_normal, "købsmoms debit");
    }
}
