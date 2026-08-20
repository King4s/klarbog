//! MCP Revolut OAuth start + callback (HTTP parity; never echo tokens).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_bank::{
    oauth_exchange_code, oauth_start, RevolutOAuthConfig, RevolutOAuthError,
};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

fn map_oauth(err: RevolutOAuthError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

/// Mirror GET /api/v1/revolut/oauth/start — auth URL + state; never returns secrets.
pub async fn revolut_oauth_start(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    if let Err(e) = authorize_company(allowlist_root, &company, &actor).await {
        return map_core_error(e);
    }
    let cfg = match RevolutOAuthConfig::from_env() {
        Ok(c) => c,
        Err(e) => return map_oauth(RevolutOAuthError::Config(e)),
    };
    let start = oauth_start(&cfg);
    Envelope::ok(json!({
        "auth_url": start.auth_url,
        "state": start.state,
    }))
}

/// Mirror POST /api/v1/revolut/oauth/callback — code exchange; stores secrets; never echoes tokens.
pub async fn revolut_oauth_callback(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let code = match args.get("code").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return map_oauth(RevolutOAuthError::MissingCode),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let cfg = match RevolutOAuthConfig::from_env() {
        Ok(c) => c,
        Err(e) => return map_oauth(RevolutOAuthError::Config(e)),
    };
    match oauth_exchange_code(&cfg, &path, code).await {
        Ok(()) => Envelope::ok(json!({
            "stored": true,
            "path": "secrets/revolut.json",
        })),
        Err(e) => map_oauth(e),
    }
}

#[cfg(test)]
#[path = "bank_oauth_tests.rs"]
mod tests;
