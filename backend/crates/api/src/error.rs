//! Application error type and its HTTP representation.
//!
//! [`AppError`] is the single error type returned by handlers. It implements
//! [`IntoResponse`] so every error is serialized as a consistent JSON envelope:
//!
//! ```json
//! { "error": { "code": "not_found", "message": "…" } }
//! ```

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// The crate-wide error type. Handlers return `Result<T, AppError>`.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("{0}")]
    BadRequest(String),

    #[error("service unavailable: {0}")]
    Unavailable(String),

    // Boxed to keep `AppError` small: `surrealdb::Error` is ~144 bytes, and
    // leaving it inline would bloat every `Result<_, AppError>` in the codebase.
    #[error("database error")]
    Database(Box<surrealdb::Error>),

    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl From<surrealdb::Error> for AppError {
    fn from(err: surrealdb::Error) -> Self {
        AppError::Database(Box::new(err))
    }
}

impl AppError {
    /// The HTTP status this error maps to.
    fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Database(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// A stable, machine-readable error code for clients.
    fn code(&self) -> &'static str {
        match self {
            AppError::NotFound => "not_found",
            AppError::BadRequest(_) => "bad_request",
            AppError::Unavailable(_) => "unavailable",
            AppError::Database(_) => "database_error",
            AppError::Internal(_) => "internal_error",
        }
    }

    /// The client-safe message. Internal/DB error details are logged, never
    /// leaked in the response body. Callers constructing `Unavailable`/
    /// `BadRequest` are responsible for keeping *their* message client-safe.
    fn public_message(&self) -> String {
        match self {
            AppError::Database(_) => "an internal database error occurred".to_string(),
            AppError::Internal(_) => "an internal error occurred".to_string(),
            other => other.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();

        // Server-side faults are logged with their full cause; the client only
        // ever sees the sanitized `public_message`.
        if status.is_server_error() {
            tracing::error!(error = %self, code = self.code(), "request failed");
        } else {
            tracing::debug!(error = %self, code = self.code(), "request rejected");
        }

        let body = ErrorBody {
            error: ErrorDetail {
                code: self.code(),
                message: self.public_message(),
            },
        };

        (status, Json(body)).into_response()
    }
}

/// Convenience alias for handler results.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn status_and_code_mapping() {
        assert_eq!(AppError::NotFound.status(), StatusCode::NOT_FOUND);
        assert_eq!(AppError::NotFound.code(), "not_found");

        let bad = AppError::BadRequest("nope".into());
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
        assert_eq!(bad.code(), "bad_request");

        let unavail = AppError::Unavailable("down".into());
        assert_eq!(unavail.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(unavail.code(), "unavailable");
    }

    #[tokio::test]
    async fn internal_error_body_is_sanitized() {
        // The source carries a secret-looking detail that must NOT reach the client.
        let secret = "sensitive stack detail at 10.0.0.5:8000";
        let err = AppError::Internal(Box::new(std::io::Error::other(secret)));

        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["error"]["code"], "internal_error");
        assert_eq!(body["error"]["message"], "an internal error occurred");
        assert!(
            !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "internal error source leaked into the response body"
        );
    }
}
