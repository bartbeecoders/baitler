//! Baitler HTTP API library.
//!
//! The binary (`main.rs`) is a thin wrapper around [`run`]. Exposing the app as a
//! library lets integration tests (and future workspace crates) build and drive
//! the server in-process. See the module docs for each concern:
//! [`config`], [`db`], [`error`], [`migrations`], [`routes`], [`state`],
//! [`telemetry`].

pub mod config;
pub mod db;
pub mod error;
pub mod migrations;
pub mod routes;
pub mod state;
pub mod telemetry;

pub use config::Config;
pub use state::AppState;

use axum::Router;
use tokio::net::TcpListener;

/// Build fully-wired [`AppState`]: connect to the database and apply migrations.
///
/// Returns a boxed error so the two distinct failure sources — connecting
/// ([`surrealdb::Error`]) and migrating ([`migrations::MigrationError`]) — share
/// one signature.
pub async fn build_state(
    config: Config,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let db = db::connect(&config.surreal).await?;
    migrations::run(&db).await?;
    Ok(AppState::new(config, db))
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
