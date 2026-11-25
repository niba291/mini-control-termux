use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct AppState {
    root: Arc<PathBuf>,
}

impl AppState {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: Arc::new(root),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
