//! Offline HTTP contract smoke (slice 35 + 39 + wave8).
//!
//! Uses axum `oneshot` against a temp company — no TCP bind, no network.

mod flow;
mod moms_currency;
mod oauth;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_types::{Actor, ActorKind, Envelope};
use serde_json::Value;
use tower::ServiceExt;

pub(super) use crate::ENV_TEST_LOCK;

const PAYOUT_FIXTURE: &str =
    include_str!("../../../klarbog-plugin-bank/tests/fixtures/stripe_webhook_payout_paid.json");
const TEST_SECRET: &str = "whsec_test_fixture_secret";

fn actor_headers(actor: &Actor) -> (&'static str, String) {
    let kind = match actor.kind {
        ActorKind::User => "user",
        ActorKind::Agent => "agent",
        ActorKind::System => "system",
    };
    (kind, actor.id.clone())
}

async fn json_envelope(
    app: axum::Router,
    method: &str,
    uri: &str,
    kind: &str,
    id: &str,
    body: Value,
) -> (StatusCode, Envelope<Value>) {
    let res = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let env: Envelope<Value> = serde_json::from_slice(&bytes).unwrap();
    (status, env)
}

async fn json_req(
    app: axum::Router,
    method: &str,
    uri: &str,
    kind: &str,
    id: &str,
    body: Value,
) -> (StatusCode, Value) {
    let (status, env) = json_envelope(app, method, uri, kind, id, body).await;
    (status, env.data.unwrap_or(Value::Null))
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
