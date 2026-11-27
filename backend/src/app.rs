use axum::{
    http::{Method, Uri},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tower_http::trace::TraceLayer;

use crate::{
    core::error::AppError,
    modules,
    state::AppState,
};

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
        endpoints: vec![
            "/",
            "/api/health",
            "/api/modules",
            "/api/files",
            "/api/files/content?path=...",
            "PUT /api/files/content?path=...",
        ],
    })
}

async fn not_found(uri: Uri) -> AppError {
    AppError::NotFound(format!("route not found: {uri}"))
}

async fn method_not_allowed(method: Method, uri: Uri) -> AppError {
    AppError::MethodNotAllowed(format!("method {method} not allowed for {uri}"))
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .nest("/api", modules::router())
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
