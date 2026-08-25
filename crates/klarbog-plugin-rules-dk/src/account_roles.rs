//! Account-role compatibility and claim-income resolution (mirrors TS `account-roles.ts`
//! + `invoice-claim-receivable.ts` for the Rust chart-backed product).

use crate::chart::{find_account, AccountType};
use thiserror::Error;

/// Semantic roles used when posting invoice claims (reminder / interest / compensation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountRole {
    Bank,
    Debtors,
    Creditors,
    OutputVat,
    InputVat,
    ReverseChargeVat,
    VatSettlement,
    OperationalDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AccountRoleError {
    #[error("account {account_no} does not exist")]
    NotFound { account_no: String },
    #[error("account {account_no} is not compatible with role '{role}'")]
    Incompatible {
        account_no: String,
        role: &'static str,
    },
    #[error(
        "account {account_no} is not an active, directly postable credit-normal income account"
    )]
    InvalidClaimIncome { account_no: String },
}

struct RoleExpectation {
    role_label: &'static str,
    account_type: AccountType,
    credit_normal: bool,
}

const fn expectation(role: AccountRole) -> RoleExpectation {
    match role {
        AccountRole::Bank => RoleExpectation {
            role_label: "bank",
            account_type: AccountType::Asset,
            credit_normal: false,
        },
        AccountRole::Debtors => RoleExpectation {
            role_label: "debtors",
            account_type: AccountType::Asset,
            credit_normal: false,
        },
        AccountRole::Creditors => RoleExpectation {
            role_label: "creditors",
            account_type: AccountType::Liability,
            credit_normal: true,
        },
        AccountRole::OutputVat => RoleExpectation {
            role_label: "output_vat",
            account_type: AccountType::Vat,
            credit_normal: true,
        },
        AccountRole::InputVat => RoleExpectation {
            role_label: "input_vat",
            account_type: AccountType::Vat,
            credit_normal: false,
        },
        AccountRole::ReverseChargeVat => RoleExpectation {
            role_label: "reverse_charge_vat",
            account_type: AccountType::Vat,
            credit_normal: true,
        },
        AccountRole::VatSettlement => RoleExpectation {
            role_label: "vat_settlement",
            account_type: AccountType::Liability,
            credit_normal: true,
        },
        AccountRole::OperationalDefault => RoleExpectation {
            role_label: "operational_default",
            account_type: AccountType::Expense,
            credit_normal: false,
        },
    }
}

fn chart_row(account_no: &str) -> Result<i64, AccountRoleError> {
    let trimmed = account_no.trim();
    if trimmed.is_empty() {
        return Err(AccountRoleError::NotFound {
            account_no: "(empty)".into(),
        });
    }
    trimmed
        .parse::<i64>()
        .map_err(|_| AccountRoleError::NotFound {
            account_no: trimmed.into(),
        })
}

/// Read-only preflight: account metadata must match the role's semantic expectations.
pub fn account_role_compatibility(
    account_no: &str,
    role: AccountRole,
) -> Result<(), AccountRoleError> {
    let code = chart_row(account_no)?;
    let row = find_account(code).ok_or_else(|| AccountRoleError::NotFound {
        account_no: account_no.trim().into(),
    })?;
    let expected = expectation(role);
    if row.account_type != expected.account_type || row.credit_normal != expected.credit_normal {
        return Err(AccountRoleError::Incompatible {
            account_no: account_no.trim().into(),
            role: expected.role_label,
        });
    }
    Ok(())
}

/// Resolve the only account class a claim-origin credit may use (default `1010`).
pub fn resolve_claim_income_account(
    requested_account_no: &str,
) -> Result<String, AccountRoleError> {
    let account_no = if requested_account_no.trim().is_empty() {
        "1010"
    } else {
        requested_account_no.trim()
    };
    let code = chart_row(account_no)?;
    let row = find_account(code).ok_or_else(|| AccountRoleError::NotFound {
        account_no: account_no.into(),
    })?;
    if row.account_type != AccountType::Income || !row.credit_normal {
        return Err(AccountRoleError::InvalidClaimIncome {
            account_no: account_no.into(),
        });
    }
    Ok(account_no.to_string())
}

/// Resolve receivable account for claim postings (must be debtors-compatible).
pub fn resolve_debtors_account(requested_account_no: &str) -> Result<String, AccountRoleError> {
    let account_no = if requested_account_no.trim().is_empty() {
        "1100"
    } else {
        requested_account_no.trim()
    };
    account_role_compatibility(account_no, AccountRole::Debtors)?;
    Ok(account_no.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_chart_roles_are_compatible() {
        for (account, role) in [
            ("2000", AccountRole::Bank),
            ("1100", AccountRole::Debtors),
            ("7000", AccountRole::Creditors),
            ("1200", AccountRole::OutputVat),
            ("4000", AccountRole::InputVat),
            ("1200", AccountRole::ReverseChargeVat),
            ("4500", AccountRole::VatSettlement),
            ("3000", AccountRole::OperationalDefault),
        ] {
            account_role_compatibility(account, role)
                .unwrap_or_else(|e| panic!("{account}/{role:?}: {e}"));
        }
    }

    #[test]
    fn bank_role_rejects_output_vat_account() {
        let err = account_role_compatibility("1200", AccountRole::Bank).unwrap_err();
        assert!(matches!(err, AccountRoleError::Incompatible { .. }));
    }

    #[test]
    fn resolve_claim_income_defaults_to_1010() {
        assert_eq!(resolve_claim_income_account("").unwrap(), "1010");
        assert_eq!(resolve_claim_income_account("1010").unwrap(), "1010");
    }

    #[test]
    fn resolve_claim_income_rejects_expense_account() {
        let err = resolve_claim_income_account("3000").unwrap_err();
        assert!(matches!(err, AccountRoleError::InvalidClaimIncome { .. }));
    }

    #[test]
    fn resolve_claim_income_rejects_unknown_account() {
        let err = resolve_claim_income_account("9999").unwrap_err();
        assert!(matches!(err, AccountRoleError::NotFound { .. }));
    }

    #[test]
    fn resolve_debtors_rejects_revenue_account() {
        let err = resolve_debtors_account("1000").unwrap_err();
        assert!(matches!(err, AccountRoleError::Incompatible { .. }));
    }
}
