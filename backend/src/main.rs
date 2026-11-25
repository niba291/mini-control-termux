mod app;
mod config;
mod core;
mod modules;
mod state;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = config::Config::from_env()?;
    let state = state::AppState::new(config.root);
    let app = app::create_app(state);
    let listener = TcpListener::bind(config.addr).await?;
    let addr = listener.local_addr()?;

    info!("Pocket Server running on http://{addr}");

    axum::serve(listener, app).await?;

    Ok(())
}
