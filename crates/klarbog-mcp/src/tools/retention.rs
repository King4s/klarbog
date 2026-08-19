//! Retention read + backup manifest MCP tools.

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_retention::{
    load_retention, manifest_path, manifest_sidecar_path, write_backup_manifest, BackupError,
    RetentionError,
};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn map_retention(err: RetentionError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn map_backup(err: BackupError) -> Envelope<Value> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_core::init_company;
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
}
