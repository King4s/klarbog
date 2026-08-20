//! Shared MCP tool helpers (allowlist root + status payload).

use klarbog_plugin::{Capability, Registry};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn default_allowlist_root() -> PathBuf {
    std::env::var("KLARBOG_ALLOWLIST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Mirror GET `/api/v1/status` — mode, bind surface, allowlist, plugin roster.
pub fn status_payload(allowlist_root: &Path, registry: &Registry) -> Value {
    let cap_name = |c: Capability| match c {
        Capability::Read => "read",
        Capability::CrmWrite => "crm_write",
        Capability::JournalWrite => "journal_write",
        Capability::RulesValidate => "rules_validate",
    };
    let plugins: Vec<_> = registry
        .list()
        .map(|p| {
            json!({
                "id": p.id(),
                "version": p.version(),
                "capabilities": p.capabilities().iter().map(|c| cap_name(*c)).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "mode": "dev",
        "bind": "stdio",
        "allowlist_root": allowlist_root,
        "plugins": plugins,
    })
}
