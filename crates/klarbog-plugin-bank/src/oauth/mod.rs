//! Revolut OAuth token flow (ADR-008 scaffold — no full UI).

mod revolut;
mod token_store;

#[cfg(test)]
mod revolut_tests;

pub use revolut::{
    from_env_or_company_secrets_refreshed, oauth_exchange_code, oauth_exchange_code_with_client,
    oauth_start, oauth_start_with_state, refresh_access_token, refresh_access_token_with_client,
    RevolutOAuthError, RevolutOAuthStart, DEFAULT_AUTH_URL,
};
pub use token_store::{
    load_revolut_access_token, load_revolut_tokens, save_revolut_tokens, RevolutStoredTokens,
};
