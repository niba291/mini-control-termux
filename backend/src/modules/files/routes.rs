use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self, File},
    io::{AsyncWriteExt, copy},
};
use tokio_util::io::{ReaderStream, StreamReader};

use crate::{core::error::AppError, state::AppState};

#[derive(Deserialize)]
struct PathQuery {
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

/// Join a client path against the root. Relative paths are resolved against
/// the root; absolute paths are accepted and validated later.
fn resolve(root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

async fn list_dir(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> Result<Json<ListResponse>, AppError> {
    let root = state.root();
    let requested = match query.path.as_deref() {
        Some(path) if !path.is_empty() => resolve(root, path),
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

async fn download(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> Result<Response, AppError> {
    let root = state.root();
    let path = query
        .path
        .as_deref()
        .filter(|path| !path.is_empty())
        .ok_or_else(|| AppError::BadRequest("missing `path` query parameter".to_string()))?;

    let requested = resolve(root, path);
    let file_path = fs::canonicalize(&requested)
        .await
        .map_err(|err| io_error(err, &requested))?;

    if !file_path.starts_with(root) {
        return Err(AppError::Forbidden(format!(
            "path outside the allowed root: {}",
            requested.display()
        )));
    }

    let metadata = fs::metadata(&file_path)
        .await
        .map_err(|err| io_error(err, &file_path))?;

    if metadata.is_dir() {
        return Err(AppError::BadRequest(format!(
            "not a file: {}",
            file_path.display()
        )));
    }

    let file = File::open(&file_path)
        .await
        .map_err(|err| io_error(err, &file_path))?;

    let content_type = content_type(&file_path);
    let name = file_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());

    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = response.headers_mut();

    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&metadata.len().to_string()) {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    if let Ok(value) = HeaderValue::from_str(&content_disposition(
        &name,
        content_type != "application/octet-stream",
    )) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }

    Ok(response)
}

#[derive(Serialize)]
struct UploadResponse {
    path: String,
    size: u64,
}

async fn upload(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
    body: Body,
) -> Result<(StatusCode, Json<UploadResponse>), AppError> {
    let root = state.root();
    let path = query
        .path
        .as_deref()
        .filter(|path| !path.is_empty())
        .ok_or_else(|| AppError::BadRequest("missing `path` query parameter".to_string()))?;

    let requested = resolve(root, path);
    let name = requested
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty() && name != "." && name != "..")
        .ok_or_else(|| AppError::BadRequest("path must include a file name".to_string()))?;

    let parent = requested
        .parent()
        .ok_or_else(|| AppError::BadRequest("path must include a file name".to_string()))?;
    let parent = fs::canonicalize(parent)
        .await
        .map_err(|err| io_error(err, parent))?;

    if !parent.starts_with(root) {
        return Err(AppError::Forbidden(format!(
            "path outside the allowed root: {}",
            requested.display()
        )));
    }

    let destination = parent.join(&name);

    if fs::try_exists(&destination)
        .await
        .map_err(|err| io_error(err, &destination))?
    {
        return Err(AppError::Conflict(format!(
            "path already exists: {}",
            destination.display()
        )));
    }

    // Write to a temporary file in the same directory and rename on success,
    // so a failed upload never leaves a partial file behind.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = parent.join(format!(".{name}.upload-{}-{stamp}", std::process::id()));

    let result = async {
        let mut file = File::create_new(&temp)
            .await
            .map_err(|err| io_error(err, &temp))?;

        let mut reader =
            StreamReader::new(body.into_data_stream().map_err(std::io::Error::other));
        let size = copy(&mut reader, &mut file)
            .await
            .map_err(|err| io_error(err, &temp))?;

        file.flush().await.map_err(|err| io_error(err, &temp))?;
        drop(file);

        fs::rename(&temp, &destination)
            .await
            .map_err(|err| io_error(err, &destination))?;

        Ok::<u64, AppError>(size)
    }
    .await;

    let size = match result {
        Ok(size) => size,
        Err(err) => {
            let _ = fs::remove_file(&temp).await;
            return Err(err);
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(UploadResponse {
            path: destination.to_string_lossy().into_owned(),
            size,
        }),
    ))
}

fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match extension.as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("txt" | "log" | "md" | "csv" | "yaml" | "yml" | "toml" | "ini" | "rs" | "py") => {
            "text/plain; charset=utf-8"
        }
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("pdf") => "application/pdf",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("m4a") => "audio/mp4",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("tar") => "application/x-tar",
        Some("7z") => "application/x-7z-compressed",
        Some("apk") => "application/vnd.android.package-archive",
        _ => "application/octet-stream",
    }
}

/// Build an RFC 6266 `Content-Disposition` header that supports non-ASCII
/// names (common in Termux paths) by percent-encoding the UTF-8 filename.
fn content_disposition(name: &str, inline: bool) -> String {
    let disposition = if inline { "inline" } else { "attachment" };
    let mut encoded = String::with_capacity(name.len());

    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    format!("{disposition}; filename*=UTF-8''{encoded}")
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
    Router::new()
        .route("/", get(list_dir))
        .route("/content", get(download).put(upload))
}
