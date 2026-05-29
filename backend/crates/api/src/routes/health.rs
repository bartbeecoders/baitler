//! Health and version endpoints.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    db: &'static str,
}

/// `GET /health` — readiness probe. Pings the database within a bounded timeout
/// (so a slow/half-open datastore can't hang the probe); returns 200 when the
/// service can serve traffic, 503 otherwise.
///
/// The client-facing message is deliberately static: the underlying database
/// error (which can carry host/port/protocol detail) is logged, never returned,
/// since this endpoint is unauthenticated.
pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    match tokio::time::timeout(state.config.db_timeout, db::ping(&state.db)).await {
        Ok(Ok(())) => Ok(Json(HealthResponse {
            status: "ok",
            db: "up",
        })),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "health check: database ping failed");
            Err(AppError::Unavailable("database not reachable".to_string()))
        }
        Err(_elapsed) => {
            tracing::warn!(timeout = ?state.config.db_timeout, "health check: database ping timed out");
            Err(AppError::Unavailable("database not reachable".to_string()))
        }
    }
}

#[derive(Serialize)]
pub struct VersionResponse {
    name: &'static str,
    version: &'static str,
    /// Git commit SHA, when injected at build time via the `GIT_SHA` env var.
    git_sha: Option<&'static str>,
}

/// `GET /version` — build/version metadata.
pub async fn version() -> Json<VersionResponse> {
    Json(VersionResponse {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        git_sha: option_env!("GIT_SHA"),
    })
}
