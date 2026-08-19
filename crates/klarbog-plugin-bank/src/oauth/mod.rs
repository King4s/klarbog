//! Revolut OAuth token flow (ADR-008 scaffold — no full UI).

mod revolut;
mod token_store;

pub use revolut::{
    oauth_exchange_code, oauth_exchange_code_with_client, oauth_start, oauth_start_with_state,
    RevolutOAuthError, RevolutOAuthStart, DEFAULT_AUTH_URL,
};
pub use token_store::{
    load_revolut_access_token, load_revolut_tokens, save_revolut_tokens, RevolutStoredTokens,
};
