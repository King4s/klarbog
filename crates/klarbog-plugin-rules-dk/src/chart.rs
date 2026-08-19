//! Klarbog DEV chart-of-accounts stub (Danish numeric codes).
//!
//! Not a full SKAT chart — only codes Klarbog plugins use by default, plus the
//! expense band used by `dk.expense.*` rules.
//!
//! | Code / range | Role |
//! |---|---|
//! | `1000` | Bank (invoice / payment default) |
//! | `1500` | Accounts receivable (AR) |
//! | `4400` | Accounts payable (AP) |
//! | `4000`–`6999` | Expense band (also covers bank-CSV `5800` / income `6100` in DEV) |
//!
//! Digits outside this stub still validate as account strings; rules-dk only
//! emits optional hint [`RULE_KNOWN_ACCOUNT`] (`dk.bookkeeping.known_account`).

/// Bank (invoice / payment suggestion default).
pub const DK_CHART_BANK: i64 = 1_000;
/// Accounts receivable.
pub const DK_CHART_AR: i64 = 1_500;
/// Accounts payable.
pub const DK_CHART_AP: i64 = 4_400;
/// Inclusive start of expense stub band.
pub const DK_CHART_EXPENSE_MIN: i64 = 4_000;
/// Inclusive end of expense stub band.
pub const DK_CHART_EXPENSE_MAX: i64 = 6_999;

/// Applied-rule id when a digit account is outside the stub allowlist (hint only).
pub const RULE_KNOWN_ACCOUNT: &str = "dk.bookkeeping.known_account";

/// True when `n` is in the documented expense stub band.
#[inline]
pub fn is_expense_account_code(n: i64) -> bool {
    (DK_CHART_EXPENSE_MIN..=DK_CHART_EXPENSE_MAX).contains(&n)
}

/// True when the account string parses as i64 and matches the stub allowlist.
pub fn is_known_dk_account(account: &str) -> bool {
    let Ok(n) = account.parse::<i64>() else {
        return false;
    };
    n == DK_CHART_BANK || n == DK_CHART_AR || n == DK_CHART_AP || is_expense_account_code(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_codes_are_known() {
        for code in [
            "1000", "1500", "4400", "4000", "6000", "6999", "5800", "6100",
        ] {
            assert!(is_known_dk_account(code), "{code}");
        }
    }

    #[test]
    fn outside_stub_not_known() {
        for code in ["2000", "3000", "7000", "9999", "1"] {
            assert!(!is_known_dk_account(code), "{code}");
        }
    }

    #[test]
    fn non_digit_not_known() {
        assert!(!is_known_dk_account("6abc"));
        assert!(!is_known_dk_account(""));
    }
}
