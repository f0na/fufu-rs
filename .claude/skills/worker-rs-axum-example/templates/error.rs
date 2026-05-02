// src/error.rs - Custom Error Types

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Application error enum for handling different error types
#[derive(Debug)]
pub enum AppError {
    /// Worker runtime errors
    Worker(worker::Error),
    /// Client error (400)
    BadRequest(String),
    /// Authentication error (401)
    Unauthorized(String),
    /// Resource not found (404)
    NotFound(String),
    /// Permission denied (403)
    Forbidden(String),
    /// Server error (500)
    Internal(String),
}

// Implement From for automatic error conversion
impl From<worker::Error> for AppError {
    fn from(e: worker::Error) -> Self {
        AppError::Worker(e)
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        AppError::Unauthorized(e.to_string())
    }
}

// Convert AppError into HTTP response
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Worker(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}