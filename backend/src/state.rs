
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
}

impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }
}
