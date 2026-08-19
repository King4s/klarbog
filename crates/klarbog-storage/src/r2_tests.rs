//! R2 store tests.

use crate::r2::*;
use crate::r2_time::format_amz_date;
use crate::sigv4::{encode_s3_object_path, sign_s3_request, SignInput};
use crate::StorageError;

fn sample_parts(
    jurisdiction: &str,
    allow_non_eu: bool,
) -> (String, String, String, String, String, bool) {
    (
        "acct".into(),
        "key-id".into(),
        "secret".into(),
        "bucket".into(),
        jurisdiction.into(),
        allow_non_eu,
    )
}

fn eu_config() -> R2Config {
    let (a, k, s, b, j, allow) = sample_parts("eu", false);
    R2Config::from_parts(a, k, s, b, j, allow).unwrap()
}

#[test]
fn eu_endpoint_default_jurisdiction() {
    let ep = build_r2_endpoint("abc123", "eu");
    assert_eq!(ep, "https://abc123.eu.r2.cloudflarestorage.com");
    assert!(is_eu_jurisdiction("eu", &ep));
}

#[test]
fn non_eu_rejected_without_override() {
    let (a, k, s, b, j, allow) = sample_parts("us", false);
    assert_eq!(
        R2Config::from_parts(a, k, s, b, j, allow),
        Err(StorageError::NonEuJurisdiction)
    );
}

#[test]
fn non_eu_allowed_with_override_flag() {
    let (a, k, s, b, j, allow) = sample_parts("us", true);
    let cfg = R2Config::from_parts(a, k, s, b, j, allow).unwrap();
    assert!(!is_eu_jurisdiction(&cfg.jurisdiction, &cfg.endpoint));
    assert!(cfg.allow_non_eu);
}

#[test]
fn empty_bucket_rejected() {
    let err = R2Config::from_parts(
        "acct".into(),
        "k".into(),
        "s".into(),
        "  ".into(),
        "eu".into(),
        false,
    )
    .unwrap_err();
    assert!(matches!(err, StorageError::InvalidR2Config(_)));
}

#[test]
fn object_url_and_signing_offline() {
    let store = R2Store::new(eu_config());
    let (url, signed) = store
        .sign_for("PUT", "attachments/x.pdf", b"data", "20260820T100000Z")
        .unwrap();
    assert_eq!(
        url,
        "https://acct.eu.r2.cloudflarestorage.com/bucket/attachments/x.pdf"
    );
    assert!(signed
        .authorization
        .contains("Credential=key-id/20260820/auto/s3/aws4_request"));
}

#[test]
fn format_amz_date_known_epoch() {
    assert_eq!(format_amz_date(1_787_220_000), "20260820T100000Z");
}

#[tokio::test]
async fn put_rejects_invalid_key_without_network() {
    let store = R2Store::new(eu_config());
    assert_eq!(
        store.put("../escape", b"x").await,
        Err(StorageError::ParentDirInKey)
    );
}

#[tokio::test]
async fn get_rejects_empty_key_without_network() {
    let store = R2Store::new(eu_config());
    assert_eq!(store.get("  ").await, Err(StorageError::EmptyKey));
}

#[test]
fn delete_signing_offline() {
    let store = R2Store::new(eu_config());
    let (url, signed) = store
        .sign_for("DELETE", "attachments/x.pdf", b"", "20260820T100000Z")
        .unwrap();
    assert_eq!(
        url,
        "https://acct.eu.r2.cloudflarestorage.com/bucket/attachments/x.pdf"
    );
    assert!(signed
        .authorization
        .contains("Credential=key-id/20260820/auto/s3/aws4_request"));
}

#[tokio::test]
async fn delete_rejects_invalid_key_without_network() {
    let store = R2Store::new(eu_config());
    assert_eq!(
        store.delete("../escape").await,
        Err(StorageError::ParentDirInKey)
    );
}

#[test]
fn canonical_uri_matches_sigv4_module() {
    let cfg = eu_config();
    let host = cfg.host().unwrap();
    let uri = encode_s3_object_path(&cfg.bucket, "attachments/a b.pdf");
    let signed = sign_s3_request(&SignInput {
        method: "PUT",
        host: &host,
        canonical_uri: &uri,
        payload: b"hello",
        access_key_id: &cfg.access_key_id,
        secret_access_key: &cfg.secret_access_key,
        region: R2_SIGV4_REGION,
        amz_date: "20260820T100000Z",
    });
    assert!(signed.authorization.starts_with("AWS4-HMAC-SHA256"));
}
