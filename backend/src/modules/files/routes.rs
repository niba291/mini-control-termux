use std::{
    path::Path,
    time::UNIX_EPOCH,
};

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{core::error::AppError, state::AppState};

#[derive(Deserialize)]
struct ListQuery {
    path: Option<String>,
}

#[derive(Serialize)]
struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: Option<u64>,
}

#[derive(Serialize)]
struct ListResponse {
    path: String,
    entries: Vec<FileEntry>,
}

async fn list_dir(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ListResponse>, AppError> {
    let root = state.root();

    let requested = match query.path.as_deref() {
        Some(path) if !path.is_empty() => {
            let path = Path::new(path);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            }
        }
        _ => root.to_path_buf(),
    };

    // Canonicalizing resolves `..`, symlinks and relative segments, so the
    // root check below is done against the real target.
    let dir = fs::canonicalize(&requested)
        .await
        .map_err(|err| io_error(err, &requested))?;

    if !dir.starts_with(root) {
        return Err(AppError::Forbidden(format!(
            "path outside the allowed root: {}",
            requested.display()
        )));
    }

    let metadata = fs::metadata(&dir)
        .await
        .map_err(|err| io_error(err, &dir))?;

    if !metadata.is_dir() {
        return Err(AppError::BadRequest(format!(
            "not a directory: {}",
            dir.display()
        )));
    }

    let mut read_dir = fs::read_dir(&dir).await.map_err(|err| io_error(err, &dir))?;
    let mut entries = Vec::new();

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|err| io_error(err, &dir))?
    {
        let path = entry.path();
        let file_type = entry.file_type().await.map_err(|err| io_error(err, &path))?;
        // Neither `file_type` nor `entry.metadata()` follow symlinks, so a
        // symlink is listed as a regular entry. If the client later requests
        // it, `canonicalize` resolves the target and the root check applies.
        let metadata = entry.metadata().await.ok();

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            is_dir: file_type.is_dir(),
            size: if file_type.is_dir() {
                0
            } else {
                metadata.as_ref().map_or(0, |m| m.len())
            },
            modified: metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        });
    }

    // Directories first, then case-insensitive name.
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(Json(ListResponse {
        path: dir.to_string_lossy().into_owned(),
        entries,
    }))
}

fn io_error(err: std::io::Error, path: &Path) -> AppError {
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            AppError::NotFound(format!("path not found: {}", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => {
            AppError::Forbidden(format!("permission denied: {}", path.display()))
        }
        std::io::ErrorKind::NotADirectory => {
            AppError::BadRequest(format!("not a directory: {}", path.display()))
        }
        _ => AppError::Internal(format!("{}: {err}", path.display())),
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_dir))
}
