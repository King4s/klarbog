//! HTTP integration tests for klarbog-api (slice 2).

use crate::{default_state, router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_types::{Actor, Currency, Envelope, MinorAmount};
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

fn sample_entry(actor: Actor, minor: i64) -> JournalEntry {
    let amount = MinorAmount::from_minor(minor);
    let currency = Currency::new("DKK").unwrap();
    JournalEntry {
        as_of: Utc::now(),
        memo: "api test #receipt".into(),
        actor: actor.clone(),
        legs: vec![
            Leg {
                account: "6000".into(),
                direction: Direction::Debit,
                amount,
                currency: currency.clone(),
                party_id: None,
            },
            Leg {
                account: "5800".into(),
                direction: Direction::Credit,
                amount,
                currency,
                party_id: None,
            },
        ],
    }
}

fn actor_headers(actor: &Actor) -> (&'static str, String) {
    let kind = match actor.kind {
        klarbog_types::ActorKind::User => "user",
        klarbog_types::ActorKind::Agent => "agent",
        klarbog_types::ActorKind::System => "system",
    };
    (kind, actor.id.clone())
}

#[tokio::test]
async fn health_ok() {
    let app = router(default_state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn status_includes_allowlist() {
    let dir = tempdir().unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn preview_commit_posts() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let entry = sample_entry(owner.clone(), 300);
    let (kind, id) = actor_headers(&owner);
    let preview_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "entry": &entry,
    });
    let preview_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/journal/preview")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(preview_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview_res.status(), StatusCode::OK);
    let preview_bytes = axum::body::to_bytes(preview_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let preview_env: Envelope<Value> = serde_json::from_slice(&preview_bytes).unwrap();
    assert!(preview_env.ok);
    assert!(!preview_env
        .applied_rules
        .contains(&"dk.expense.receipt_hint".to_string()));
    let token = preview_env.data.unwrap()["confirm_token"]
        .as_str()
        .unwrap()
        .to_string();
    let commit_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "entry": &entry,
        "confirm_token": token,
    });
    let commit_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/journal/commit")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(commit_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(commit_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_token_fails() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let entry = sample_entry(owner.clone(), 100);
    let (kind, id) = actor_headers(&owner);
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "entry": entry,
        "confirm_token": "not-a-real-token",
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/journal/commit")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn actor_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let intruder = Actor::user("intruder");
    let entry = sample_entry(intruder.clone(), 100);
    let (kind, id) = actor_headers(&intruder);
    let body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "entry": entry,
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/journal/preview")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn path_outside_allowlist_fails() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let entry = sample_entry(owner.clone(), 100);
    let (kind, id) = actor_headers(&owner);
    let body = serde_json::json!({
        "company": "/tmp/outside-klarbog-co",
        "entry": entry,
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/journal/preview")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn missing_actor_headers_rejected() {
    let app = router(default_state());
    let body = serde_json::json!({
            "company": "/tmp/klarbog-outside-x",
        "entry": sample_entry(Actor::user("x"), 1),
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/journal/preview")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
