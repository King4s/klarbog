use super::*;
use klarbog_core::init_company;
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn mcp_documents_and_exceptions_flow() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();

    let attach = documents_attach(
        &json!({
            "company": company_path.to_string_lossy(),
            "kind": "receipt",
            "path_hint": "attachments/r.pdf",
            "notes": "fuel",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(attach.ok, "{attach:?}");
    let doc = attach.data.unwrap();
    let doc_id = doc["id"].as_str().unwrap().to_string();
    assert_eq!(doc["kind"], "receipt");
    assert_eq!(doc["path_hint"], "attachments/r.pdf");

    let listed = documents_list(
        &json!({
            "company": company_path.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(listed.ok, "{listed:?}");
    assert_eq!(listed.data.unwrap().as_array().unwrap().len(), 1);

    let got = documents_list(
        &json!({
            "company": company_path.to_string_lossy(),
            "document_id": doc_id,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(got.ok, "{got:?}");
    assert_eq!(got.data.unwrap()["notes"], "fuel");

    let raised = exceptions_raise(
        &json!({
            "company": company_path.to_string_lossy(),
            "code": "missing_attachment",
            "severity": "warn",
            "message": "No scan linked",
            "related_ids": [doc_id.clone()],
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(raised.ok, "{raised:?}");
    let exc_id = raised.data.unwrap()["id"].as_str().unwrap().to_string();

    let open_list = exceptions_list(
        &json!({
            "company": company_path.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(open_list.ok, "{open_list:?}");
    assert_eq!(open_list.data.unwrap().as_array().unwrap().len(), 1);

    let closed = exceptions_set_open(
        &json!({
            "company": company_path.to_string_lossy(),
            "exception_id": exc_id,
            "open": false,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(closed.ok, "{closed:?}");
    assert_eq!(closed.data.unwrap()["open"], false);

    let after_close = exceptions_list(
        &json!({
            "company": company_path.to_string_lossy(),
            "open_only": true,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(after_close.ok, "{after_close:?}");
    assert!(after_close.data.unwrap().as_array().unwrap().is_empty());

    let deleted = documents_delete(
        &json!({
            "company": company_path.to_string_lossy(),
            "document_id": doc_id,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(deleted.ok, "{deleted:?}");
    let empty = documents_list(
        &json!({
            "company": company_path.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(empty.ok, "{empty:?}");
    assert!(empty.data.unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn mcp_documents_reject_path_traversal() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let env = documents_attach(
        &json!({
            "company": company_path.to_string_lossy(),
            "kind": "other",
            "path_hint": "../bad.pdf",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(!env.ok);
}

#[tokio::test]
async fn mcp_exceptions_empty_code_rejected() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let env = exceptions_raise(
        &json!({
            "company": company_path.to_string_lossy(),
            "code": "   ",
            "severity": "info",
            "message": "x",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(!env.ok);
}

#[tokio::test]
async fn mcp_documents_authz_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let env = documents_list(
        &json!({
            "company": company_path.to_string_lossy(),
            "actor_kind": "agent",
            "actor_id": "stranger",
        }),
        dir.path(),
    )
    .await;
    assert!(!env.ok);
}
