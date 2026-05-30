use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("playlist not found")]
    NotFound,

    #[error("playlist is not public")]
    Forbidden,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error("rss serialization failed: {0}")]
    Rss(String),
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    timestamp: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "PLAYLIST_NOT_FOUND"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "PLAYLIST_NOT_PUBLIC"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::Database(err) => {
                tracing::error!(error = ?err, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
            }
            AppError::Rss(_) => {
                tracing::error!(error = %self, "rss serialization error");
                (StatusCode::INTERNAL_SERVER_ERROR, "RSS_BUILD_FAILED")
            }
        };

        let body = ErrorBody {
            code,
            message: self.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
