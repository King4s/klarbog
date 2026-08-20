//! CRM parties MCP tools — mirror POST/GET /api/v1/crm/parties (no journal write).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_crm::{get_party, list_parties, upsert_party, CrmError};
use klarbog_types::{Envelope, PartyId};
use serde_json::Value;
use std::path::Path;

fn map_crm(err: CrmError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

/// Mirror POST /api/v1/crm/parties — create or update party; never writes journal.
pub async fn crm_upsert_party(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let display_name = match args.get("display_name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return Envelope::err(["missing display_name"]),
    };
    let party_id = args
        .get("party_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| PartyId::new(s.to_string()));
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match upsert_party(&path, party_id, display_name) {
        Ok(party) => Envelope::ok(serde_json::to_value(party).unwrap()),
        Err(e) => map_crm(e),
    }
}

/// Mirror GET /api/v1/crm/parties — list all, or one when `party_id` set.
pub async fn crm_list_parties(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
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
    if let Some(raw_id) = args
        .get("party_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let id = PartyId::new(raw_id.to_string());
        return match get_party(&path, &id) {
            Ok(Some(party)) => Envelope::ok(serde_json::to_value(party).unwrap()),
            Ok(None) => Envelope::err([format!("party not found: {id}")]),
            Err(e) => map_crm(e),
        };
    }
    match list_parties(&path) {
        Ok(parties) => Envelope::ok(serde_json::to_value(parties).unwrap()),
        Err(e) => map_crm(e),
    }
}

#[cfg(test)]
#[path = "crm_tests.rs"]
mod tests;
