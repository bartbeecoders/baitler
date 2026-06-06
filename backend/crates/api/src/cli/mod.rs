//! Claude Code CLI integration (Phase 13).
//!
//! Wraps the `claude` CLI as a server-side, headless, sandboxed agent runner
//! that streams back to the portal and loops back through Baitler's own `/mcp`,
//! so the agent operates on the user's own knowledge base. Off by default
//! (`CLAUDE_CLI_ENABLED`); the [`MockRunner`] (selected by `CLAUDE_BIN == "mock"`)
//! is the offline, CI-tested path while the real [`ClaudeCliRunner`] is compiled
//! but unexercised here (it needs network egress + an Anthropic key).
//!
//! Layout mirrors `documents/`/`pages/`: [`runner`] (the trait + [`RunSpec`]),
//! [`events`] ([`RunEvent`]), [`model`] (the row + DTOs), [`repo`] (persistence),
//! [`routes`] (the owner-scoped HTTP surface), plus [`claude`]/[`mock`]
//! implementations and a [`registry`] of active runs.

pub mod claude;
pub mod events;
pub mod mock;
pub mod model;
pub mod registry;
pub mod repo;
pub mod routes;
pub mod runner;

use std::net::SocketAddr;
use std::sync::Arc;

use crate::config::{CliConfig, McpConfig};

pub use claude::ClaudeCliRunner;
pub use events::RunEvent;
pub use mock::MockRunner;
pub use registry::RunRegistry;
pub use runner::{AgentProvider, AgentRunner, RunError, RunSpec, RunStream, ToolScope};

/// Agent label attributed to the runner's loopback MCP writes + activity rows.
pub const AGENT_LABEL: &str = "claude-code";

// Root-jailed path resolution lives in [`crate::workspace`] (shared with the
// MCP `workspace_*` tools); re-exported here for the run-grant callers.
pub use crate::workspace::{resolve_under_roots, validate_workspace};

/// Emit a single startup line describing CLI-runner readiness, so a misconfigured
/// `claude` (disabled, wrong PATH) is diagnosable from the logs without a run.
pub async fn log_startup(cli: &CliConfig, runner: &dyn AgentRunner) {
    if !cli.enabled {
        tracing::info!("Claude Code CLI runner: disabled (set CLAUDE_CLI_ENABLED=true to enable)");
        return;
    }
    let health = runner.health().await;
    if health.binary_ok {
        tracing::info!(
            kind = runner.kind(),
            bin = %cli.bin,
            version = ?health.version,
            "Claude Code CLI runner: ready"
        );
    } else {
        tracing::warn!(
            bin = %cli.bin,
            detail = ?health.detail,
            "Claude Code CLI runner: ENABLED but `claude` is not runnable — runs will fail \
             until CLAUDE_BIN points at an installed CLI on the server PATH"
        );
    }
}

/// Build the configured runner. The `"mock"` binary sentinel selects the
/// in-process [`MockRunner`] (no subprocess, no egress); anything else builds the
/// real [`ClaudeCliRunner`], whose loopback MCP URL defaults to this process's
/// own `/mcp` on loopback.
pub fn build_runner(
    cli: &CliConfig,
    mcp: &McpConfig,
    bind_addr: SocketAddr,
) -> Arc<dyn AgentRunner> {
    if cli.bin == "mock" {
        Arc::new(MockRunner)
    } else {
        let loopback = cli
            .mcp_loopback_url
            .clone()
            .unwrap_or_else(|| format!("http://127.0.0.1:{}/mcp", bind_addr.port()));
        Arc::new(ClaudeCliRunner::new(
            cli.clone(),
            mcp.auth_token.clone(),
            loopback,
        ))
    }
}
