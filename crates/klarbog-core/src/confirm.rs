//! Two-phase confirmation tokens (Claude IMPORTANT).

use klarbog_types::KlarbogError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmToken {
    pub token: String,
    pub expires_unix_ms: u128,
}

struct Pending {
    payload_digest: String,
    expires: Instant,
}

pub struct ConfirmStore {
    inner: Mutex<HashMap<String, Pending>>,
    ttl: Duration,
}

impl Default for ConfirmStore {
    fn default() -> Self {
        Self::new(Duration::from_secs(300))
    }
}

impl ConfirmStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Phase 1: bind an opaque token to a payload digest (not self-satisfiable confirm:true).
    pub fn issue(&self, payload_digest: &str) -> ConfirmToken {
        let mut h = Sha256::new();
        h.update(payload_digest.as_bytes());
        h.update(uuid_like());
        let token = hex::encode(h.finalize());
        let expires = Instant::now() + self.ttl;
        self.inner.lock().expect("confirm lock").insert(
            token.clone(),
            Pending {
                payload_digest: payload_digest.to_string(),
                expires,
            },
        );
        ConfirmToken {
            token,
            expires_unix_ms: 0, // clients should treat TTL as server-side
        }
    }

    /// Phase 2: consume token iff it matches the same payload digest.
    pub fn consume(&self, token: &str, payload_digest: &str) -> Result<(), KlarbogError> {
        let mut map = self.inner.lock().expect("confirm lock");
        let Some(pending) = map.remove(token) else {
            return Err(KlarbogError::ConfirmRequired);
        };
        if Instant::now() > pending.expires {
            return Err(KlarbogError::Message("confirm token expired".into()));
        }
        if pending.payload_digest != payload_digest {
            return Err(KlarbogError::Message(
                "confirm token payload mismatch".into(),
            ));
        }
        Ok(())
    }
}

fn uuid_like() -> [u8; 16] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&(nanos as u64).to_le_bytes());
    out[8..].copy_from_slice(&(nanos.rotate_left(17) as u64).to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_phase_roundtrip() {
        let s = ConfirmStore::default();
        let t = s.issue("digest-a");
        assert!(s.consume(&t.token, "digest-a").is_ok());
        assert!(s.consume(&t.token, "digest-a").is_err());
    }

    #[test]
    fn mismatch_fails() {
        let s = ConfirmStore::default();
        let t = s.issue("digest-a");
        assert!(s.consume(&t.token, "digest-b").is_err());
    }
}
