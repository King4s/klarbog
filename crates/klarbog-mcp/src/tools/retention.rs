//! Retention read + backup + GDPR erase + retention purge MCP tools.

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_retention::{
    erase_party, load_retention, manifest_path, manifest_sidecar_path, run_retention_purge,
    write_backup_manifest, write_gdpr_export, BackupError, ErasePartyOptions, GdprError,
    PurgeError, PurgeOptions, RetentionError,
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

fn map_purge(err: PurgeError) -> Envelope<Value> {
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

/// Mirror POST /api/v1/gdpr-export — company-scoped metadata only; writes gdpr_export.json.
pub async fn gdpr_export(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
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
    match write_gdpr_export(&path) {
        Ok(export) => Envelope::ok(serde_json::to_value(export).unwrap()),
        Err(e) => map_gdpr(e),
    }
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

/// Mirror POST /api/v1/retention/purge — dry-run unless confirm:true; journal untouched.
pub async fn retention_purge(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let confirm = args
        .get("confirm")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let gc_orphan_documents = args
        .get("gc_orphan_documents")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match run_retention_purge(
        &path,
        PurgeOptions {
            confirm,
            gc_orphan_documents,
        },
    )
    .await
    {
        Ok(report) => Envelope::ok(serde_json::to_value(report).unwrap()),
        Err(e) => map_purge(e),
    }
}

#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
