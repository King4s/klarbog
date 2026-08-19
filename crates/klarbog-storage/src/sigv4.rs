//! Minimal AWS SigV4 signing for S3-compatible APIs (Cloudflare R2, ADR-007).

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const ALGORITHM: &str = "AWS4-HMAC-SHA256";
const SERVICE: &str = "s3";
const TERMINAL: &str = "aws4_request";

/// Signed HTTP headers for an S3-compatible request (no network I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedHeaders {
    pub host: String,
    pub x_amz_date: String,
    pub x_amz_content_sha256: String,
    pub authorization: String,
}

/// Inputs for [`sign_s3_request`]. `amz_date` must be `YYYYMMDDTHHMMSSZ`.
pub struct SignInput<'a> {
    pub method: &'a str,
    pub host: &'a str,
    pub canonical_uri: &'a str,
    pub payload: &'a [u8],
    pub access_key_id: &'a str,
    pub secret_access_key: &'a str,
    pub region: &'a str,
    pub amz_date: &'a str,
}

/// Sign an S3 PutObject/GetObject/DeleteObject-style request (path-style URI, empty query).
pub fn sign_s3_request(input: &SignInput<'_>) -> SignedHeaders {
    let payload_hash = sha256_hex(input.payload);
    let date_stamp = &input.amz_date[..8];
    let credential_scope = format!("{date_stamp}/{}/{SERVICE}/{TERMINAL}", input.region);

    let canonical_headers = format!(
        "host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n",
        host = input.host,
        payload_hash = payload_hash,
        amz_date = input.amz_date,
    );
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";

    let canonical_request = format!(
        "{method}\n{uri}\n\n{headers}{signed}\n{payload_hash}",
        method = input.method,
        uri = input.canonical_uri,
        headers = canonical_headers,
        signed = signed_headers,
        payload_hash = payload_hash,
    );

    let string_to_sign = format!(
        "{ALGORITHM}\n{amz_date}\n{scope}\n{hash}",
        amz_date = input.amz_date,
        scope = credential_scope,
        hash = sha256_hex(canonical_request.as_bytes()),
    );

    let signing_key = derive_signing_key(input.secret_access_key, date_stamp, input.region);
    let signature = hmac_sha256_hex(&signing_key, string_to_sign.as_bytes());

    let authorization = format!(
        "{ALGORITHM} Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        access_key = input.access_key_id,
        scope = credential_scope,
        signed_headers = signed_headers,
        signature = signature,
    );

    SignedHeaders {
        host: input.host.to_string(),
        x_amz_date: input.amz_date.to_string(),
        x_amz_content_sha256: payload_hash,
        authorization,
    }
}

/// Percent-encode each path segment for S3 canonical URI (slashes preserved).
pub fn encode_s3_object_path(bucket: &str, key: &str) -> String {
    let bucket_enc = uri_encode_segment(bucket);
    let key_enc = key
        .split('/')
        .map(uri_encode_segment)
        .collect::<Vec<_>>()
        .join("/");
    format!("/{bucket_enc}/{key_enc}")
}

fn uri_encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    hex::encode(hmac_sha256(key, data))
}

fn derive_signing_key(secret: &str, date_stamp: &str, region: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, SERVICE.as_bytes());
    hmac_sha256(&k_service, TERMINAL.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_hash_is_known_constant() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn encodes_spaces_and_slashes_in_key() {
        assert_eq!(
            encode_s3_object_path("my-bucket", "attachments/a b.pdf"),
            "/my-bucket/attachments/a%20b.pdf"
        );
    }

    #[test]
    fn signing_is_deterministic_for_fixed_clock() {
        let uri = encode_s3_object_path("klarbog-dev", "attachments/receipt.pdf");
        let signed = sign_s3_request(&SignInput {
            method: "PUT",
            host: "abc123.eu.r2.cloudflarestorage.com",
            canonical_uri: &uri,
            payload: b"hello-r2",
            access_key_id: "test-access-key",
            secret_access_key: "test-secret-key",
            region: "auto",
            amz_date: "20260820T100000Z",
        });
        assert!(signed
            .authorization
            .starts_with("AWS4-HMAC-SHA256 Credential=test-access-key/"));
        assert!(signed
            .authorization
            .contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"));
        assert_eq!(signed.x_amz_date, "20260820T100000Z");
        assert_eq!(signed.x_amz_content_sha256, sha256_hex(b"hello-r2"));
    }

    #[test]
    fn get_uses_empty_body_hash() {
        let uri = encode_s3_object_path("b", "k");
        let signed = sign_s3_request(&SignInput {
            method: "GET",
            host: "x.eu.r2.cloudflarestorage.com",
            canonical_uri: &uri,
            payload: b"",
            access_key_id: "ak",
            secret_access_key: "sk",
            region: "auto",
            amz_date: "20260820T100000Z",
        });
        assert_eq!(
            signed.x_amz_content_sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
