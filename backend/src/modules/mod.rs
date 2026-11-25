
use axum::{
    routing::get,
    Json,
    Router,
};

use serde::Serialize;

pub mod health;

use crate::{
    core::modules::{registered_modules, ModuleInfo},
    state::AppState,
};

#[derive(Serialize)]
struct ModulesResponse {
    modules: Vec<ModuleInfo>,
}

async fn list_modules() -> Json<ModulesResponse> {
    Json(ModulesResponse {
        modules: registered_modules(),
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/health", health::routes::router())
        .route("/modules", get(list_modules))
}
