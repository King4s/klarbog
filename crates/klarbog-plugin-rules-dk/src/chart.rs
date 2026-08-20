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

use serde::Serialize;

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

/// One stub chart row for HTTP/MCP read surfaces (codes + labels).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChartStubEntry {
    /// Code or inclusive range string (`1000`, `4000-6999`).
    pub code: String,
    /// Human label (English DEV stub).
    pub label: &'static str,
    /// Inclusive range start when `code` is a band.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    /// Inclusive range end when `code` is a band.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

/// Read-only list of documented stub codes + labels (no journal I/O).
pub fn chart_stub_entries() -> Vec<ChartStubEntry> {
    vec![
        ChartStubEntry {
            code: DK_CHART_BANK.to_string(),
            label: "Bank",
            min: None,
            max: None,
        },
        ChartStubEntry {
            code: DK_CHART_AR.to_string(),
            label: "Accounts receivable",
            min: None,
            max: None,
        },
        ChartStubEntry {
            code: DK_CHART_AP.to_string(),
            label: "Accounts payable",
            min: None,
            max: None,
        },
        ChartStubEntry {
            code: format!("{DK_CHART_EXPENSE_MIN}-{DK_CHART_EXPENSE_MAX}"),
            label: "Expense band",
            min: Some(DK_CHART_EXPENSE_MIN),
            max: Some(DK_CHART_EXPENSE_MAX),
        },
    ]
}

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

    #[test]
    fn chart_stub_entries_codes_and_labels() {
        let entries = chart_stub_entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].code, "1000");
        assert_eq!(entries[0].label, "Bank");
        assert_eq!(entries[1].code, "1500");
        assert_eq!(entries[2].code, "4400");
        assert_eq!(entries[3].code, "4000-6999");
        assert_eq!(entries[3].min, Some(DK_CHART_EXPENSE_MIN));
        assert_eq!(entries[3].max, Some(DK_CHART_EXPENSE_MAX));
    }
}
