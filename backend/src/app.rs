use axum::Router;
use tower_http::trace::TraceLayer;

use crate::{
    modules,
    state::AppState,
};

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .nest("/api", modules::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
