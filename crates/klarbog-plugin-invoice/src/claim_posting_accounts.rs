//! Validate receivable + claim-income accounts before journal suggestions.

use crate::{InvoiceConfig, InvoiceError};
use klarbog_plugin_rules_dk::{
    resolve_claim_income_account, resolve_debtors_account, AccountRoleError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedClaimAccounts {
    pub receivable_account: String,
    pub income_account: String,
}

pub(crate) fn resolve_claim_posting_accounts(
    cfg: &InvoiceConfig,
) -> Result<ResolvedClaimAccounts, InvoiceError> {
    let receivable_account = resolve_debtors_account(&cfg.ar_account).map_err(map_role)?;
    let income_account =
        resolve_claim_income_account(&cfg.interest_income_account).map_err(map_role)?;
    Ok(ResolvedClaimAccounts {
        receivable_account,
        income_account,
    })
}

fn map_role(err: AccountRoleError) -> InvoiceError {
    InvoiceError::InvalidClaimAccount(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InvoiceConfig;

    #[test]
    fn default_config_resolves_native_accounts() {
        let resolved = resolve_claim_posting_accounts(&InvoiceConfig::default()).unwrap();
        assert_eq!(resolved.receivable_account, "1100");
        assert_eq!(resolved.income_account, "1010");
    }

    #[test]
    fn incompatible_receivable_is_rejected() {
        let cfg = InvoiceConfig {
            ar_account: "1000".into(),
            ..InvoiceConfig::default()
        };
        assert!(matches!(
            resolve_claim_posting_accounts(&cfg),
            Err(InvoiceError::InvalidClaimAccount(_))
        ));
    }

    #[test]
    fn incompatible_income_is_rejected() {
        let cfg = InvoiceConfig {
            interest_income_account: "1100".into(),
            ..InvoiceConfig::default()
        };
        assert!(matches!(
            resolve_claim_posting_accounts(&cfg),
            Err(InvoiceError::InvalidClaimAccount(_))
        ));
    }
}
