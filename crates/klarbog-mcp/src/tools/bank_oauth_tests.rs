//! Tests for Revolut OAuth MCP start/callback (fail-closed; never assert raw tokens).

use super::{revolut_oauth_callback, revolut_oauth_start};
use klarbog_core::init_company;
use klarbog_types::Actor;
use serde_json::json;
use tempfile::tempdir;
struct EnvGuard {
    key: &'static str,
    prev: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, val: &str) -> Self {
        let prev = std::env::var(key).ok();
        unsafe { std::env::set_var(key, val) };
        Self { key, prev }
    }

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

fn set_oauth_env() -> Vec<EnvGuard> {
    vec![
        EnvGuard::set("KLARBOG_REVOLUT_CLIENT_ID", "client-id"),
        EnvGuard::set("KLARBOG_REVOLUT_CLIENT_SECRET", "client-secret"),
        EnvGuard::set(
            "KLARBOG_REVOLUT_REDIRECT_URI",
            "https://example.test/callback",
        ),
        EnvGuard::set("KLARBOG_REVOLUT_TOKEN_URL", "https://example.test/token"),
        EnvGuard::set("KLARBOG_REVOLUT_AUTH_URL", "https://example.test/auth"),
    ]
}

#[tokio::test]
async fn mcp_oauth_start_returns_auth_url_without_secrets() {
    let _lock = super::super::ENV_TEST_LOCK.lock().await;
    let _env = set_oauth_env();
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = revolut_oauth_start(&args, dir.path()).await;
    assert!(env.ok);
    let body = serde_json::to_string(&env).unwrap();
    let data = env.data.as_ref().unwrap();
    let url = data["auth_url"].as_str().unwrap();
    assert!(url.contains("client_id=client-id"));
    assert!(url.contains("response_type=code"));
    assert!(!url.is_empty());
    assert!(!data["state"].as_str().unwrap().is_empty());
    assert!(!body.contains("client-secret"));
    assert!(!body.contains("\"access_token\""));
    assert!(!body.contains("\"refresh_token\""));
}

#[tokio::test]
async fn mcp_oauth_start_fail_closed_without_config() {
    let _lock = super::super::ENV_TEST_LOCK.lock().await;
    let _clear = [
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_ID"),
        EnvGuard::unset("KLARBOG_REVOLUT_CLIENT_SECRET"),
        EnvGuard::unset("KLARBOG_REVOLUT_REDIRECT_URI"),
    ];
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = revolut_oauth_start(&args, dir.path()).await;
    assert!(!env.ok);
    let body = serde_json::to_string(&env).unwrap();
    assert!(!body.contains("\"access_token\""));
}

#[tokio::test]
async fn mcp_oauth_callback_missing_code_fail_closed() {
    let _lock = super::super::ENV_TEST_LOCK.lock().await;
    let _env = set_oauth_env();
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = revolut_oauth_callback(&args, dir.path()).await;
    assert!(!env.ok);
    assert!(
        env.errors[0].contains("code") || env.errors[0].contains("Missing"),
        "{}",
        env.errors[0]
    );
    let body = serde_json::to_string(&env).unwrap();
    assert!(!body.contains("client-secret"));
}

#[tokio::test]
async fn mcp_oauth_callback_blank_code_fail_closed() {
    let _lock = super::super::ENV_TEST_LOCK.lock().await;
    let _env = set_oauth_env();
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "code": "   ",
        "actor_kind": "user",
        "actor_id": "owner",
    });
    let env = revolut_oauth_callback(&args, dir.path()).await;
    assert!(!env.ok);
    assert!(
        env.errors[0].contains("code") || env.errors[0].contains("Missing"),
        "{}",
        env.errors[0]
    );
}

#[tokio::test]
async fn mcp_oauth_start_actor_denied() {
    let _lock = super::super::ENV_TEST_LOCK.lock().await;
    let _env = set_oauth_env();
    let dir = tempdir().unwrap();
    let owner = Actor::user("owner");
    let company_path = dir.path().join("co");
    init_company(&company_path, "Demo", &owner).await.unwrap();
    let args = json!({
        "company": company_path.to_string_lossy(),
        "actor_kind": "user",
        "actor_id": "intruder",
    });
    let env = revolut_oauth_start(&args, dir.path()).await;
    assert!(!env.ok);
    assert!(env.errors[0].contains("actor not in policy"));
}
