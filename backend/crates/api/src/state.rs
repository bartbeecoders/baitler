//! Shared application state passed to every handler.

use std::sync::Arc;

use crate::cli::{AgentRunner, RunRegistry};
use crate::config::Config;
use crate::db::Db;
use crate::llm::LlmRegistry;
use crate::plugins::PluginRegistry;
use crate::storage::Storage;

/// Cloneable handle to shared resources. `Clone` is cheap: it bumps the inner
/// `Arc` refcounts. Injected into handlers via axum's `State` extractor.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub storage: Arc<dyn Storage>,
    pub llm: Arc<LlmRegistry>,
    /// Claude Code CLI runner (Phase 13): the mock or the real subprocess
    /// runner, selected from `config.cli`.
    pub cli_runner: Arc<dyn AgentRunner>,
    /// Active-run registry (per-owner serialisation + cancellation).
    pub cli_runs: Arc<RunRegistry>,
    /// Plugin registry (Phase 16): MCP tools contributed by enabled plugins.
    /// Constructed empty; the Phase 16.B loader populates it in `build_state`.
    pub plugins: Arc<PluginRegistry>,
}

impl AppState {
    pub fn new(config: Config, db: Db, storage: Arc<dyn Storage>) -> Self {
        let cli_runner = crate::cli::build_runner(&config.cli, &config.mcp, config.bind_addr);
        Self {
            config: Arc::new(config),
            db,
            storage,
            llm: Arc::new(LlmRegistry::with_defaults()),
            cli_runner,
            cli_runs: Arc::new(RunRegistry::default()),
            plugins: Arc::new(PluginRegistry::default()),
        }
    }
}
