use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("insufficient balance: {0}")]
    InsufficientBalance(String),
    #[error("invalid transfer: {0}")]
    InvalidTransfer(String),
    #[error("idempotency conflict: {0}")]
    IdempotencyConflict(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::Database(msg) => {
                tracing::error!("database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    String::from("ERR_DB"),
                    String::from("internal server database issue"),
                )
            }
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                String::from("ERR_NOT_FOUND"),
                msg,
            ),
            AppError::InsufficientBalance(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                String::from("ERR_INSUFFICIENT_BALANCE"),
                msg,
            ),
            AppError::InvalidTransfer(msg) => (
                StatusCode::BAD_REQUEST,
                String::from("ERR_INVALID_TRANSFER"),
                msg,
            ),
            AppError::IdempotencyConflict(msg) => (
                StatusCode::CONFLICT,
                String::from("ERR_IDEMPOTENCY_CONFLICT"),
                msg,
            ),
        };

        let body = ErrorDetail { code, message };

        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        AppError::Database(value.to_string())
    }
}
