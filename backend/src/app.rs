use axum::{routing::get, Json, Router};
use serde::Serialize;
use tower_http::trace::TraceLayer;

use crate::{modules, state::AppState};

#[derive(Serialize)]
struct ServerInfo {
    name: &'static str,
    version: &'static str,
    endpoints: Vec<&'static str>,
}

async fn root() -> Json<ServerInfo> {
    Json(ServerInfo {
        name: "Pocket Server",
        version: env!("CARGO_PKG_VERSION"),
        endpoints: vec!["/", "/api/health", "/api/modules"],
    })
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .nest("/api", modules::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
