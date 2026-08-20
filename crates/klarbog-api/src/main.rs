//! Klarbog HTTP API — default loopback `127.0.0.1:3195` (ADR-003 / ADR-014).
//!
//! Non-loopback bind only when `KLARBOG_ALLOW_NON_LOOPBACK=1` **and** explicit
//! `KLARBOG_BIND=...`. Fail-closed otherwise. Never enabled by default.

use std::env;
use std::net::SocketAddr;

const DEFAULT_BIND: &str = "127.0.0.1:3195";
const ENV_BIND: &str = "KLARBOG_BIND";
const ENV_ALLOW_NON_LOOPBACK: &str = "KLARBOG_ALLOW_NON_LOOPBACK";

#[derive(Debug, PartialEq, Eq)]
enum BindError {
    InvalidBind(String),
    NonLoopbackRefused(String),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBind(raw) => {
                write!(f, "invalid {ENV_BIND} value {raw:?} (expected host:port)")
            }
            Self::NonLoopbackRefused(raw) => write!(
                f,
                "refusing non-loopback bind {raw:?}: set {ENV_ALLOW_NON_LOOPBACK}=1 and explicit {ENV_BIND} (ADR-014)"
            ),
        }
    }
}

impl std::error::Error for BindError {}

fn env_truthy(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => {
            let lower = v.trim().to_ascii_lowercase();
            lower == "1" || lower == "true" || lower == "yes"
        }
        Err(_) => false,
    }
}

/// Resolve listen address from optional bind string + allow flag (ADR-014).
///
/// - No / empty bind → `127.0.0.1:3195`
/// - Loopback bind → always OK
/// - Non-loopback → only if `allow_non_loopback`
fn resolve_bind_addr(
    bind_env: Option<&str>,
    allow_non_loopback: bool,
) -> Result<SocketAddr, BindError> {
    let raw = bind_env.map(str::trim).filter(|s| !s.is_empty());
    let Some(raw) = raw else {
        return DEFAULT_BIND
            .parse()
            .map_err(|_| BindError::InvalidBind(DEFAULT_BIND.into()));
    };
    let addr: SocketAddr = raw
        .parse()
        .map_err(|_| BindError::InvalidBind(raw.to_string()))?;
    if !addr.ip().is_loopback() && !allow_non_loopback {
        return Err(BindError::NonLoopbackRefused(raw.to_string()));
    }
    Ok(addr)
}

fn resolve_bind_from_env() -> Result<SocketAddr, BindError> {
    let bind = env::var(ENV_BIND).ok();
    let allow = env_truthy(ENV_ALLOW_NON_LOOPBACK);
    resolve_bind_addr(bind.as_deref(), allow)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let addr = resolve_bind_from_env().map_err(|e| anyhow::anyhow!("{e}"))?;
    tracing::info!("klarbog-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, klarbog_api::router(klarbog_api::default_state())).await?;
    Ok(())
}

#[cfg(test)]
mod bind_gate_tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn default_is_loopback_3195() {
        let addr = resolve_bind_addr(None, false).unwrap();
        assert_eq!(addr, "127.0.0.1:3195".parse().unwrap());
        let addr = resolve_bind_addr(Some(""), false).unwrap();
        assert_eq!(addr.port(), 3195);
        assert!(addr.ip().is_loopback());
    }

    #[test]
    fn loopback_bind_ok_without_allow_flag() {
        let addr = resolve_bind_addr(Some("127.0.0.1:4000"), false).unwrap();
        assert_eq!(addr.port(), 4000);
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));

        let addr = resolve_bind_addr(Some("[::1]:3195"), false).unwrap();
        assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
    }

    #[test]
    fn non_loopback_refused_without_allow_flag() {
        let err = resolve_bind_addr(Some("0.0.0.0:3195"), false).unwrap_err();
        assert!(matches!(err, BindError::NonLoopbackRefused(_)));

        let err = resolve_bind_addr(Some("192.168.1.10:3195"), false).unwrap_err();
        assert!(matches!(err, BindError::NonLoopbackRefused(_)));
    }

    #[test]
    fn non_loopback_ok_with_allow_flag_and_explicit_bind() {
        let addr = resolve_bind_addr(Some("0.0.0.0:3195"), true).unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert_eq!(addr.port(), 3195);
    }

    #[test]
    fn allow_flag_alone_keeps_default_loopback() {
        // Flag without explicit bind must not widen the listen surface.
        let addr = resolve_bind_addr(None, true).unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 3195);
    }

    #[test]
    fn invalid_bind_string_errors() {
        let err = resolve_bind_addr(Some("not-a-socket"), false).unwrap_err();
        assert!(matches!(err, BindError::InvalidBind(_)));
    }
}
