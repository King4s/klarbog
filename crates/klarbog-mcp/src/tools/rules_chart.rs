//! MCP `rules_chart_list` — mirror GET /api/v1/rules/chart (read-only stub).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_rules_dk::{chart_stub_entries, RULE_KNOWN_ACCOUNT};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

/// Mirror GET /api/v1/rules/chart — stub codes + labels; never writes journal.
pub async fn rules_chart_list(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
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

    Envelope::ok(json!({
        "stub": true,
        "accounts": chart_stub_entries(),
        "rule_known_account": RULE_KNOWN_ACCOUNT,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_core::init_company;
    use klarbog_types::Actor;
    use tempfile::tempdir;

    #[tokio::test]
    async fn mcp_rules_chart_list_stub() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = dir.path().join("co");
        init_company(&company, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = rules_chart_list(&args, dir.path()).await;
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["stub"], true);
        assert_eq!(data["accounts"][0]["code"], "1000");
        assert_eq!(data["accounts"][0]["label"], "Bank");
        assert_eq!(data["accounts"][3]["code"], "4000-6999");
        assert_eq!(data["rule_known_account"], RULE_KNOWN_ACCOUNT);
    }

    #[tokio::test]
    async fn mcp_rules_chart_list_outside_allowlist_err() {
        let dir = tempdir().unwrap();
        let args = json!({
            "company": "/tmp/not-allowed-co",
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = rules_chart_list(&args, dir.path()).await;
        assert!(!env.ok);
        assert!(env.data.is_none());
    }
}
