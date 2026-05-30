//! Baitler HTTP API library.
//!
//! The binary (`main.rs`) is a thin wrapper around [`run`]. Exposing the app as a
//! library lets integration tests (and future workspace crates) build and drive
//! the server in-process. See the module docs for each concern:
//! [`config`], [`db`], [`error`], [`migrations`], [`routes`], [`state`],
//! [`telemetry`].

pub mod activity;
pub mod ai;
pub mod config;
pub mod convert;
pub mod crypto;
pub mod db;
pub mod documents;
pub mod error;
pub mod files;
pub mod ideas;
pub mod knowledge;
pub mod llm;
pub mod mcp;
pub mod migrations;
pub mod owner;
pub mod pages;
pub mod routes;
pub mod slug;
pub mod state;
pub mod storage;
pub mod telemetry;

pub use config::Config;
pub use state::AppState;

use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;

use crate::config::StorageConfig;
use crate::storage::{local::LocalStorage, Storage};

/// Build fully-wired [`AppState`]: connect to the database, apply migrations,
/// and initialize file storage.
///
/// Returns a boxed error so the distinct failure sources (connecting, migrating,
/// storage init) share one signature.
pub async fn build_state(
    config: Config,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let db = db::connect(&config.surreal).await?;
    migrations::run(&db).await?;
    let storage = build_storage(&config.storage).await?;
    Ok(AppState::new(config, db, storage))
}

/// Construct the configured storage backend.
async fn build_storage(
    cfg: &StorageConfig,
) -> Result<Arc<dyn Storage>, Box<dyn std::error::Error + Send + Sync>> {
    match cfg.backend.as_str() {
        "local" => {
            tracing::info!(path = %cfg.local_path, "using local file storage");
            Ok(Arc::new(LocalStorage::new(&cfg.local_path).await?))
        }
        other => Err(format!("unknown STORAGE_BACKEND `{other}` (supported: local)").into()),
    }
}

/// Build the application [`Router`] from prepared state.
pub fn build_app(state: AppState) -> Router {
    routes::router(state)
}

/// Run the server to completion: load config, connect, serve until shutdown.
///
/// This is the single entrypoint shared by the binary. Returns once a graceful
/// shutdown signal (Ctrl-C / SIGTERM) has drained in-flight requests.
pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env()?;
    let bind_addr = config.bind_addr;

    let state = build_state(config).await?;
    let app = build_app(state);

    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "Baitler API listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("server shut down cleanly");
    Ok(())
}

/// Resolve when the process receives Ctrl-C or (on Unix) SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
