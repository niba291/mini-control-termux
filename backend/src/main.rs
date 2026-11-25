
mod app;
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
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .init();

    let state = state::AppState::new();
    let app = app::create_app(state);
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    info!("Pocket Server running on http://127.0.0.1:8080");

    axum::serve(listener, app).await?;

    Ok(())
}