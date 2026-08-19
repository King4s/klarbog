//! Shared AuthZ + arg parsing for MCP tools.

use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_types::{Actor, ActorKind, Envelope, KlarbogError};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn parse_actor(args: &Value) -> Result<Actor, String> {
    let kind_raw = args
        .get("actor_kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing actor_kind".to_string())?;
    let id = args
        .get("actor_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing actor_id".to_string())?;
    let kind = match kind_raw {
        "user" => ActorKind::User,
        "agent" => ActorKind::Agent,
        "system" => ActorKind::System,
        other => return Err(format!("invalid actor_kind: {other}")),
    };
    Ok(Actor {
        kind,
        id: id.to_string(),
    })
}

pub fn parse_company(args: &Value) -> Result<PathBuf, String> {
    args.get("company")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "missing company".to_string())
}

pub async fn authorize_company(
    allowlist_root: &Path,
    company: &Path,
    actor: &Actor,
) -> Result<PathBuf, CoreError> {
    let path = assert_company_path(allowlist_root, company)?;
    open_existing(&path).await?.authorize(actor)?;
    Ok(path)
}

pub fn map_core_error(err: CoreError) -> Envelope<Value> {
    match err {
        CoreError::ActorDenied(tag) => Envelope::err([format!("actor not in policy: {tag}")]),
        CoreError::Path(e) => Envelope::err([e.to_string()]),
        CoreError::Journal(e) => Envelope::err([e.to_string()]),
        CoreError::RulesViolation(msg) => Envelope::err([msg]),
        CoreError::Confirm(KlarbogError::ConfirmRequired) => {
            Envelope::err(["confirm token required or already consumed"])
        }
        CoreError::Confirm(e) => Envelope::err([e.to_string()]),
        CoreError::Store(e) => Envelope::err([e.to_string()]),
        CoreError::Other(e) => Envelope::err([e.to_string()]),
    }
}
