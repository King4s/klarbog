use super::*;
use klarbog_core::init_company;
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn mcp_crm_upsert_list_get() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let upsert = json!({
        "company": company_path.to_string_lossy(),
        "display_name": "Nordic Supply ApS",
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = crm_upsert_party(&upsert, dir.path()).await;
    assert!(env.ok, "{env:?}");
    let party = env.data.unwrap();
    let party_id = party["id"].as_str().unwrap().to_string();
    assert_eq!(party["display_name"], "Nordic Supply ApS");

    let list = crm_list_parties(
        &json!({
            "company": company_path.to_string_lossy(),
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(list.ok, "{list:?}");
    assert_eq!(list.data.unwrap().as_array().unwrap().len(), 1);

    let get = crm_list_parties(
        &json!({
            "company": company_path.to_string_lossy(),
            "party_id": party_id,
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(get.ok, "{get:?}");
    assert_eq!(get.data.unwrap()["display_name"], "Nordic Supply ApS");
}

#[tokio::test]
async fn mcp_crm_upsert_empty_name_rejected() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let env = crm_upsert_party(
        &json!({
            "company": company_path.to_string_lossy(),
            "display_name": "   ",
            "actor_kind": "user",
            "actor_id": "owner",
        }),
        dir.path(),
    )
    .await;
    assert!(!env.ok);
}

#[tokio::test]
async fn mcp_crm_authz_denied() {
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let env = crm_list_parties(
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
