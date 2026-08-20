//! Optional moms post suggestion MCP tool (preview only — no journal post).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_rules_dk::{moms_post_suggestion, MomsPostSuggestionError};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

fn map_moms(err: MomsPostSuggestionError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

/// Mirror POST /api/v1/journal/moms-suggest — suggested net+vat i64 legs; never posts.
pub async fn journal_moms_post_suggestion(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
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
    let gross_minor = match args.get("gross_minor").and_then(|v| v.as_i64()) {
        Some(g) => g,
        None => return Envelope::err(["missing gross_minor"]),
    };
    let memo = match args.get("memo").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return Envelope::err(["missing memo"]),
    };

    match moms_post_suggestion(gross_minor, memo) {
        Ok(Some(s)) => Envelope::ok(json!({
            "suggested": true,
            "gross_minor": s.gross_minor,
            "net_minor": s.net_minor,
            "vat_minor": s.vat_minor,
            "rate_bps": s.rate_bps,
            "legs": s.legs,
            "auto_post": false,
        })),
        Ok(None) => Envelope::ok(json!({
            "suggested": false,
            "auto_post": false,
            "reason": "memo has no #vat25 / moms:25 / #moms25 tag",
        })),
        Err(e) => map_moms(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_core::init_company;
    use klarbog_types::Actor;
    use tempfile::tempdir;

    #[tokio::test]
    async fn mcp_moms_suggest_vat25() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = dir.path().join("co");
        init_company(&company, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company.to_string_lossy(),
            "gross_minor": 12500,
            "memo": "buy #vat25",
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = journal_moms_post_suggestion(&args, dir.path()).await;
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["net_minor"], 10_000);
        assert_eq!(data["vat_minor"], 2_500);
        assert_eq!(data["auto_post"], false);
    }

    #[tokio::test]
    async fn mcp_moms_suggest_optional_without_tag() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = dir.path().join("co");
        init_company(&company, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company.to_string_lossy(),
            "gross_minor": 12500,
            "memo": "plain",
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = journal_moms_post_suggestion(&args, dir.path()).await;
        assert!(env.ok);
        assert_eq!(env.data.unwrap()["suggested"], false);
    }

    /// Wave17: unsupported memo VAT is fail-closed (envelope not ok; never suggests).
    #[tokio::test]
    async fn mcp_moms_suggest_unsupported_memo_rate_is_err() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = dir.path().join("co");
        init_company(&company, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company.to_string_lossy(),
            "gross_minor": 12_500_i64,
            "memo": "supplies vat:12",
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = journal_moms_post_suggestion(&args, dir.path()).await;
        assert!(!env.ok);
        assert!(env.data.is_none());
    }

    /// Wave18: negative gross is fail-closed (envelope not ok; never suggests).
    #[tokio::test]
    async fn mcp_moms_suggest_negative_gross_is_err() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company = dir.path().join("co");
        init_company(&company, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company.to_string_lossy(),
            "gross_minor": -1_i64,
            "memo": "supplies #vat25",
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = journal_moms_post_suggestion(&args, dir.path()).await;
        assert!(!env.ok);
        assert!(env.data.is_none());
    }
}
