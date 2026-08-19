//! Bank import preview MCP tool (read-only drafts).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_bank::{
    default_source_for_rail, import_preview, BankImportConfig, BankImportError, BankImportSource,
    BankProfile,
};
use klarbog_types::{Currency, Envelope};
use serde_json::{json, Value};
use std::path::Path;

fn map_import(err: BankImportError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn parse_provider(args: &Value) -> Result<BankProfile, String> {
    let raw = args
        .get("provider")
        .or_else(|| args.get("profile"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing provider".to_string())?;
    serde_json::from_value(json!(raw)).map_err(|e| format!("invalid provider: {e}"))
}

fn parse_source(args: &Value, provider: BankProfile) -> Result<BankImportSource, String> {
    match args.get("source").and_then(|v| v.as_str()) {
        Some(raw) => serde_json::from_value(json!(raw)).map_err(|e| format!("invalid source: {e}")),
        None => Ok(default_source_for_rail(provider)),
    }
}

fn resolve_currency(args: &Value) -> Result<Currency, Envelope<Value>> {
    let code = args
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("DKK");
    Currency::new(code).map_err(|e| Envelope::err([e.to_string()]))
}

pub async fn bank_import_preview(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let provider = match parse_provider(args) {
        Ok(p) => p,
        Err(e) => return Envelope::err([e]),
    };
    let source = match parse_source(args, provider) {
        Ok(s) => s,
        Err(e) => return Envelope::err([e]),
    };
    let csv = args.get("csv").and_then(|v| v.as_str());
    if matches!(source, BankImportSource::Csv) && csv.unwrap_or("").is_empty() {
        return Envelope::err(["missing csv"]);
    }
    let currency = match resolve_currency(args) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let cfg = BankImportConfig {
        currency: currency.clone(),
        ..BankImportConfig::default()
    };
    let (rows, drafts) =
        match import_preview(source, provider, csv, &cfg, &actor, Some(&path)).await {
            Ok(r) => r,
            Err(e) => return map_import(e),
        };
    let summaries: Vec<Value> = rows
        .iter()
        .zip(drafts.iter())
        .map(|(row, entry)| {
            json!({
                "memo": entry.memo,
                "amount_minor": row.amount_minor.minor(),
            })
        })
        .collect();
    Envelope::ok(json!({
        "count": summaries.len(),
        "source": source,
        "provider": provider,
        "drafts": summaries,
        "company": path.to_string_lossy(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_core::init_company;
    use klarbog_types::Actor;
    use tempfile::tempdir;

    const FIXTURE: &str = "Dato;Tekst;Beløb\n19.08.2026;Office supplies;-125,50\n20.08.2026;Customer payment;500,00\n";

    #[tokio::test]
    async fn preview_generic_dk_csv() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "provider": "generic_dk",
            "source": "csv",
            "csv": FIXTURE,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = bank_import_preview(&args, dir.path()).await;
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["count"], 2);
        assert_eq!(data["source"], "csv");
        assert_eq!(data["drafts"][0]["amount_minor"], -12550);
    }

    #[tokio::test]
    async fn preview_revolut_api_missing_env() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let _guard = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
        let args = json!({
            "company": company_path.to_string_lossy(),
            "provider": "revolut",
            "source": "api",
            "currency": "DKK",
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = bank_import_preview(&args, dir.path()).await;
        assert!(!env.ok);
        assert!(env.errors[0].contains("KLARBOG_REVOLUT_API_TOKEN"));
    }

    #[tokio::test]
    async fn preview_actor_denied() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "provider": "generic_dk",
            "source": "csv",
            "csv": FIXTURE,
            "actor_kind": "user",
            "actor_id": "intruder",
        });
        let env = bank_import_preview(&args, dir.path()).await;
        assert!(!env.ok);
        assert!(env.errors[0].contains("actor not in policy"));
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            unsafe { std::env::remove_var(key) };
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }
}
