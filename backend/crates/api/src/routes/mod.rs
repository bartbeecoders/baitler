//! HTTP routing and middleware assembly.

mod health;

use axum::extract::DefaultBodyLimit;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method};
use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::error::AppError;
use crate::state::AppState;

/// Upper bound on an MCP request body. Tool calls can carry Base64 blobs
/// (`files_write`, exports), so this is more generous than the default 2 MB.
const MCP_MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// Build the application router with all routes and middleware applied.
pub fn router(state: AppState) -> Router {
    let cors = build_cors(&state.config.cors_allowed_origins);
    let files = crate::files::routes::router(state.config.storage.max_upload_bytes);
    let ideas = crate::ideas::routes::router();
    let ai = crate::ai::routes::router();
    let documents = crate::documents::routes::router();
    let knowledge = crate::knowledge::routes::router();
    let pages = crate::pages::routes::router();
    // The MCP server reuses the same state/repos. Its routes carry their own,
    // larger body limit for Base64 tool payloads.
    let mcp =
        crate::mcp::router(&state.config.mcp).layer(DefaultBodyLimit::max(MCP_MAX_BODY_BYTES));

    Router::new()
        .route("/health", get(health::health))
        .route("/version", get(health::version))
        .merge(files)
        .merge(ideas)
        .merge(ai)
        .merge(documents)
        .merge(knowledge)
        .merge(pages)
        .merge(mcp)
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// Construct the CORS layer from the configured origins.
///
/// Credentials are allowed (the app uses cookie-based sessions from Phase 2),
/// which forbids the wildcard origin — so each origin is listed explicitly.
/// Invalid origin strings are logged and skipped rather than aborting startup.
fn build_cors(origins: &[String]) -> CorsLayer {
    let parsed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| match origin.parse::<HeaderValue>() {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(origin = %origin, "ignoring invalid CORS origin");
                None
            }
        })
        .collect();

    CorsLayer::new()
        .allow_origin(parsed)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
        .allow_credentials(true)
}

/// Fallback handler for unmatched routes, yielding the standard JSON error
/// envelope instead of an empty 404.
pub async fn not_found() -> AppError {
    AppError::NotFound
}
