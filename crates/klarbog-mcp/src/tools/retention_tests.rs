use super::*;
use klarbog_core::init_company;
use klarbog_plugin_crm::upsert_party;
use klarbog_plugin_retention::{
    ensure_company_extras, gdpr_export_path, save_retention, RetentionPolicy, DEFAULT_RETAIN_DAYS,
};
use klarbog_types::Actor;
use std::fs;
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
async fn gdpr_export_writes_metadata_file() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(&company_path, None, "Export Me".into()).unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = gdpr_export(&args, dir.path()).await;
    assert!(env.ok, "{env:?}");
    let data = env.data.unwrap();
    assert!(data["note"].as_str().unwrap().contains("immutable"));
    assert_eq!(data["parties"].as_array().unwrap().len(), 1);
    assert_eq!(data["parties"][0]["id"], party.id.to_string());
    assert_eq!(data["parties"][0]["display_name"], "Export Me");
    assert!(data["invoices"].as_array().unwrap().is_empty());
    assert!(data["retention"]["retain_days"].as_i64().is_some());
    assert!(gdpr_export_path(&company_path).exists());
}

#[tokio::test]
async fn gdpr_export_authz_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "agent",
        "actor_id": "stranger",
    });
    let env = gdpr_export(&args, dir.path()).await;
    assert!(!env.ok);
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

#[tokio::test]
async fn retention_purge_dry_run_then_confirm() {
    const MS_PER_DAY: i64 = 86_400_000;

    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    save_retention(
        &company_path,
        &RetentionPolicy {
            retain_days: DEFAULT_RETAIN_DAYS,
            purge_closed_exceptions_after_days: Some(90),
        },
    )
    .unwrap();
    let closed_ms = chrono::Utc::now().timestamp_millis() - 100 * MS_PER_DAY;
    let exc_id = "exc_stale_mcp_purge";
    let path = company_path.join("exceptions.json");
    let raw = json!({
        "exceptions": [{
            "id": exc_id,
            "severity": "info",
            "code": "stale",
            "message": "old",
            "related_ids": [],
            "open": false,
            "created_unix_ms": closed_ms,
            "closed_unix_ms": closed_ms
        }]
    });
    fs::write(&path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();

    let dry_args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let dry = retention_purge(&dry_args, dir.path()).await;
    assert!(dry.ok, "{dry:?}");
    let dry_data = dry.data.unwrap();
    assert_eq!(dry_data["dry_run"], true);
    assert_eq!(dry_data["exceptions_purged"].as_array().unwrap().len(), 1);
    let listed =
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(listed["exceptions"].as_array().unwrap().len(), 1);

    let confirm_args = json!({
        "company": company_path.to_string_lossy(),
        "confirm": true,
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let done = retention_purge(&confirm_args, dir.path()).await;
    assert!(done.ok, "{done:?}");
    let data = done.data.unwrap();
    assert_eq!(data["dry_run"], false);
    assert_eq!(data["exceptions_purged"].as_array().unwrap().len(), 1);
    let after =
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(after["exceptions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn retention_purge_authz_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "agent",
        "actor_id": "stranger",
    });
    let env = retention_purge(&args, dir.path()).await;
    assert!(!env.ok);
}
