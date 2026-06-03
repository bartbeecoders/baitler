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

/// Resolve a user-supplied local path against the server's allow-list and return
/// its canonical form. Canonicalization resolves `..`/symlinks, and
/// [`Path::starts_with`] is component-wise, so neither traversal nor a look-alike
/// sibling (`/home/bart-evil`) can escape an allowed root. Accepts files or dirs.
pub fn resolve_under_roots(roots: &[String], path: &str) -> Result<std::path::PathBuf, String> {
    if roots.is_empty() {
        return Err(
            "local file access is not enabled on this server (set CLAUDE_CLI_WORKSPACE_ROOTS)"
                .to_string(),
        );
    }
    let target = std::fs::canonicalize(path)
        .map_err(|_| format!("path not found or not accessible: {path}"))?;
    for root in roots {
        if let Ok(croot) = std::fs::canonicalize(root) {
            if target.starts_with(&croot) {
                return Ok(target);
            }
        }
    }
    Err(format!(
        "path is not within an allowed root ({})",
        roots.join(", ")
    ))
}

/// Validate a user-requested local **folder** grant (must resolve, within a root,
/// and be a directory).
pub fn validate_workspace(roots: &[String], path: &str) -> Result<std::path::PathBuf, String> {
    let target = resolve_under_roots(roots, path)?;
    if !target.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    Ok(target)
}

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

#[cfg(test)]
mod tests {
    use super::validate_workspace;

    #[test]
    fn workspace_grant_is_allow_listed() {
        let root = std::env::temp_dir().join("baitler-ws-test");
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let root_s = root.to_string_lossy().into_owned();
        let roots = vec![root_s.clone()];

        // A dir within the root is accepted (canonicalized).
        assert!(validate_workspace(&roots, &sub.to_string_lossy()).is_ok());
        // The root itself is fine.
        assert!(validate_workspace(&roots, &root_s).is_ok());
        // Outside the root is rejected.
        assert!(validate_workspace(&roots, &std::env::temp_dir().to_string_lossy()).is_err());
        // A non-existent path is rejected.
        assert!(validate_workspace(&roots, &root.join("nope").to_string_lossy()).is_err());
        // Disabled when no roots are configured.
        assert!(validate_workspace(&[], &sub.to_string_lossy()).is_err());
    }
}
