use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::error;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    #[allow(dead_code)]
    BadRequest(String),
    Forbidden(String),
    Conflict(String),
    MethodNotAllowed(String),
    #[allow(dead_code)]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
    message: String,
}

impl AppError {
    fn parts(self) -> (StatusCode, &'static str, String) {
        match self {
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            AppError::Forbidden(message) => (StatusCode::FORBIDDEN, "forbidden", message),
            AppError::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            AppError::MethodNotAllowed(message) => (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", message),
            AppError::Internal(detail) => {
                error!("internal error: {detail}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_string(),
                )
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = self.parts();
        (status, Json(ErrorResponse { error, message })).into_response()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound(message)
            | AppError::BadRequest(message)
            | AppError::Forbidden(message)
            | AppError::Conflict(message)
            | AppError::MethodNotAllowed(message)
            | AppError::Internal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AppError {}
