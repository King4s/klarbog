//! DEV-only Klarbog HTTP API — binds 127.0.0.1:3195 (ADR-003).

use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let addr = SocketAddr::from(([127, 0, 0, 1], 3195));
    tracing::info!("klarbog-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, klarbog_api::router(klarbog_api::default_state())).await?;
    Ok(())
}
