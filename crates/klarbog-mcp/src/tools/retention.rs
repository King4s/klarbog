//! Retention read + backup + GDPR erase MCP tools.

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_retention::{
    erase_party, load_retention, manifest_path, manifest_sidecar_path, write_backup_manifest,
    BackupError, ErasePartyOptions, GdprError, RetentionError,
};
use klarbog_types::{Envelope, PartyId};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn map_retention(err: RetentionError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn map_backup(err: BackupError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn map_gdpr(err: GdprError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

pub async fn retention_get(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match load_retention(&path) {
        Ok(policy) => Envelope::ok(serde_json::to_value(policy).unwrap()),
        Err(e) => map_retention(e),
    }
}

pub async fn backup_manifest(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    let manifest = match write_backup_manifest(&path).await {
        Ok(m) => m,
        Err(e) => return map_backup(e),
    };
    let manifest_file = manifest_path(&path, &manifest.backup_key);
    let sidecar_file = manifest_sidecar_path(&manifest_file);
    let file_sha256 = fs::read_to_string(&sidecar_file)
        .ok()
        .map(|s| s.trim().to_string());
    Envelope::ok(json!({
        "manifest_path": manifest_file.to_string_lossy(),
        "sidecar_path": sidecar_file.to_string_lossy(),
        "backup_key": manifest.backup_key,
        "content_sha256": manifest.content_sha256,
        "file_sha256": file_sha256,
        "file_count": manifest.files.len(),
        "journal_count": manifest.journal_digests.len(),
    }))
}

/// Mirror POST /api/v1/gdpr/erase-party — dry-run unless confirm:true; journal immutable.
pub async fn gdpr_erase_party(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let party_id = match args.get("party_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => PartyId::new(id.to_string()),
        _ => return Envelope::err(["missing party_id"]),
    };
    let confirm = args
        .get("confirm")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let delete_documents = args
        .get("delete_documents")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match erase_party(
        &path,
        &party_id,
        ErasePartyOptions {
            confirm,
            delete_documents,
        },
    )
    .await
    {
        Ok(report) => Envelope::ok(serde_json::to_value(report).unwrap()),
        Err(e) => map_gdpr(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_core::init_company;
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_retention::ensure_company_extras;
    use klarbog_types::Actor;
    use tempfile::tempdir;

    #[tokio::test]
    async fn retention_load_default() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = retention_get(&args, dir.path()).await;
        assert!(env.ok);
        assert_eq!(env.data.unwrap()["retain_days"], 1825);
    }

    #[tokio::test]
    async fn backup_writes_manifest_and_sha() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        ensure_company_extras(&company_path).unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = backup_manifest(&args, dir.path()).await;
        assert!(env.ok);
        let data = env.data.unwrap();
        assert!(data["manifest_path"]
            .as_str()
            .unwrap()
            .ends_with("manifest.json"));
        assert!(data["content_sha256"].as_str().unwrap().len() == 64);
        assert!(data["file_sha256"].as_str().unwrap().len() == 64);
    }

    #[tokio::test]
    async fn gdpr_erase_party_dry_run_then_confirm() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let party = upsert_party(&company_path, None, "Erase Me".into()).unwrap();
        let dry_args = json!({
            "company": company_path.to_string_lossy(),
            "party_id": party.id.to_string(),
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let dry = gdpr_erase_party(&dry_args, dir.path()).await;
        assert!(dry.ok);
        let dry_data = dry.data.unwrap();
        assert_eq!(dry_data["dry_run"], true);
        assert_eq!(dry_data["display_name_before"], "Erase Me");
        assert!(dry_data["journal_refs_retained"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            klarbog_plugin_crm::get_party(&company_path, &party.id)
                .unwrap()
                .unwrap()
                .display_name,
            "Erase Me"
        );

        let confirm_args = json!({
            "company": company_path.to_string_lossy(),
            "party_id": party.id.to_string(),
            "confirm": true,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let done = gdpr_erase_party(&confirm_args, dir.path()).await;
        assert!(done.ok);
        let data = done.data.unwrap();
        assert_eq!(data["dry_run"], false);
        assert_eq!(data["display_name_after"], "erased");
        assert!(data["journal_refs_retained"].is_array());
        assert_eq!(
            klarbog_plugin_crm::get_party(&company_path, &party.id)
                .unwrap()
                .unwrap()
                .display_name,
            "erased"
        );
    }

    #[tokio::test]
    async fn gdpr_erase_party_authz_denied() {
        let dir = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = dir.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let party = upsert_party(&company_path, None, "X".into()).unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "party_id": party.id.to_string(),
            "actor_kind": "agent",
            "actor_id": "stranger",
        });
        let env = gdpr_erase_party(&args, dir.path()).await;
        assert!(!env.ok);
    }

    #[tokio::test]
    async fn gdpr_erase_party_outside_allowlist() {
        let dir = tempdir().unwrap();
        let other = tempdir().unwrap();
        let owner = Actor::user("owner");
        let company_path = other.path().join("co");
        init_company(&company_path, "Demo", &owner).await.unwrap();
        let party = upsert_party(&company_path, None, "Y".into()).unwrap();
        let args = json!({
            "company": company_path.to_string_lossy(),
            "party_id": party.id.to_string(),
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = gdpr_erase_party(&args, dir.path()).await;
        assert!(!env.ok);
    }
}
