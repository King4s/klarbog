//! DEV actor headers (not spoofable past company policy).

use axum::http::{HeaderMap, StatusCode};
use klarbog_types::{Actor, ActorKind, Envelope};
use serde_json::Value;

pub fn parse_actor(headers: &HeaderMap) -> Result<Actor, (StatusCode, Envelope<Value>)> {
    let kind_raw = headers
        .get("x-klarbog-actor-kind")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Envelope::err(["missing x-klarbog-actor-kind header"]),
            )
        })?;
    let id = headers
        .get("x-klarbog-actor-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Envelope::err(["missing x-klarbog-actor-id header"]),
            )
        })?;
    let kind = match kind_raw {
        "user" => ActorKind::User,
        "agent" => ActorKind::Agent,
        "system" => ActorKind::System,
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                Envelope::err([format!("invalid x-klarbog-actor-kind: {other}")]),
            ));
        }
    };
    Ok(Actor {
        kind,
        id: id.to_string(),
    })
}
