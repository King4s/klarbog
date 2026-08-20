use crate::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use klarbog_core::{default_registry, init_company, ConfirmStore};
use klarbog_types::{Actor, ActorKind, Envelope};
use serde_json::Value;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

fn actor_headers(actor: &Actor) -> (&'static str, String) {
    let kind = match actor.kind {
        ActorKind::User => "user",
        ActorKind::Agent => "agent",
        ActorKind::System => "system",
    };
    (kind, actor.id.clone())
}

#[tokio::test]
async fn gdpr_erase_party_dry_run_then_confirm() {
    use klarbog_plugin_crm::{get_party, upsert_party};
    use klarbog_plugin_documents::{attach_document, list_documents, DocumentKind};

    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let party = upsert_party(&company_path, None, "Erase Me".into()).unwrap();
    attach_document(
        &company_path,
        DocumentKind::Receipt,
        "r.pdf".into(),
        Some(party.id.clone()),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let preview_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "party_id": party.id.to_string(),
    });
    let preview_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/gdpr/erase-party")
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
    let preview = preview_env.data.unwrap();
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["documents_stripped"].as_array().unwrap().len(), 1);
    assert_eq!(
        get_party(&company_path, &party.id)
            .unwrap()
            .unwrap()
            .display_name,
        "Erase Me"
    );

    let apply_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "party_id": party.id.to_string(),
        "confirm": true,
    });
    let apply_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/gdpr/erase-party")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(apply_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(apply_res.status(), StatusCode::OK);
    let apply_bytes = axum::body::to_bytes(apply_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let apply_env: Envelope<Value> = serde_json::from_slice(&apply_bytes).unwrap();
    let apply = apply_env.data.unwrap();
    assert_eq!(apply["dry_run"], false);
    assert_eq!(apply["display_name_after"], "erased");
    assert!(apply["journal_refs_retained"].is_array());
    assert_eq!(
        get_party(&company_path, &party.id)
            .unwrap()
            .unwrap()
            .display_name,
        "erased"
    );
    assert!(list_documents(&company_path).unwrap()[0].party_id.is_none());
}

#[tokio::test]
async fn retention_purge_dry_run_then_confirm() {
    use klarbog_plugin_documents::{
        list_exceptions, raise_exception, set_exception_open, ExceptionSeverity,
    };
    use klarbog_plugin_retention::{save_retention, RetentionPolicy, DEFAULT_RETAIN_DAYS};
    use std::fs;

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
    let exc = raise_exception(
        &company_path,
        "stale".into(),
        ExceptionSeverity::Info,
        "old".into(),
        vec![],
    )
    .unwrap();
    set_exception_open(&company_path, &exc.id, false).unwrap();
    let path = company_path.join("exceptions.json");
    let mut raw: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    raw["exceptions"][0]["closed_unix_ms"] =
        serde_json::json!(chrono::Utc::now().timestamp_millis() - 100 * MS_PER_DAY);
    fs::write(path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();

    let state = AppState {
        confirm: Arc::new(ConfirmStore::default()),
        allowlist_root: dir.path().to_path_buf(),
        registry: Arc::new(default_registry()),
        api_token: None,
        session_secret: None,
        session_cookie_secure: false,
    };
    let app = router(state);
    let (kind, id) = actor_headers(&owner);
    let preview_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
    });
    let preview_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/retention/purge")
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
    let preview_data = preview_env.data.unwrap();
    assert_eq!(preview_data["dry_run"], true);
    assert_eq!(
        preview_data["exceptions_purged"].as_array().unwrap().len(),
        1
    );
    assert_eq!(list_exceptions(&company_path, false).unwrap().len(), 1);

    let apply_body = serde_json::json!({
        "company": company_path.to_string_lossy(),
        "confirm": true,
    });
    let apply_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/retention/purge")
                .header("content-type", "application/json")
                .header("x-klarbog-actor-kind", kind)
                .header("x-klarbog-actor-id", &id)
                .body(Body::from(apply_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(apply_res.status(), StatusCode::OK);
    let apply_bytes = axum::body::to_bytes(apply_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let apply_env: Envelope<Value> = serde_json::from_slice(&apply_bytes).unwrap();
    assert_eq!(apply_env.data.unwrap()["dry_run"], false);
    assert!(list_exceptions(&company_path, false).unwrap().is_empty());
}
