//! Bank CSV import preview MCP tool (read-only drafts).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_bank::{
    draft_entries_from_rows, parse_bank_csv_with_profile, BankCsvError, BankImportConfig,
    BankMapError, BankProfile,
};
use klarbog_types::{Currency, Envelope};
use serde_json::{json, Value};
use std::path::Path;

fn map_csv(err: BankCsvError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn map_map(err: BankMapError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn parse_profile(args: &Value) -> Result<BankProfile, String> {
    let raw = args
        .get("profile")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing profile".to_string())?;
    serde_json::from_value(json!(raw)).map_err(|e| format!("invalid profile: {e}"))
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
    let profile = match parse_profile(args) {
        Ok(p) => p,
        Err(e) => return Envelope::err([e]),
    };
    let csv = match args.get("csv").and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c,
        _ => return Envelope::err(["missing csv"]),
    };
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
    let required = match profile {
        BankProfile::Revolut => Some(currency),
        BankProfile::GenericDk => None,
    };
    let rows = match parse_bank_csv_with_profile(profile, csv, required.as_ref()) {
        Ok(r) => r,
        Err(e) => return map_csv(e),
    };
    let drafts = match draft_entries_from_rows(&rows, &cfg, &actor) {
        Ok(d) => d,
        Err(e) => return map_map(e),
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
    async fn preview_generic_dk() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "profile": "generic_dk",
            "csv": FIXTURE,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = bank_import_preview(&args, dir.path()).await;
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["count"], 2);
        assert_eq!(data["drafts"][0]["amount_minor"], -12550);
    }

    #[tokio::test]
    async fn preview_actor_denied() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "profile": "generic_dk",
            "csv": FIXTURE,
            "actor_kind": "user",
            "actor_id": "intruder",
        });
        let env = bank_import_preview(&args, dir.path()).await;
        assert!(!env.ok);
        assert!(env.errors[0].contains("actor not in policy"));
    }
}
