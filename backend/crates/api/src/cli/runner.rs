//! The agent-runner abstraction (Phase 13).
//!
//! Modelled on [`crate::llm::LlmProvider`]: a single async trait with two
//! implementations — a [`MockRunner`](super::mock::MockRunner) (the offline,
//! CI-tested path) and a [`ClaudeCliRunner`](super::claude::ClaudeCliRunner)
//! that spawns the real `claude` CLI. Connection/spawn-time failures return
//! `Err` (mapped to a 503 *before* any stream begins); per-run failures surface
//! as a terminal [`RunEvent::Error`](super::events::RunEvent::Error) in the
//! stream.

use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::Stream;
use thiserror::Error;
use tokio::sync::Notify;

use super::events::RunEvent;

/// A stream of [`RunEvent`]s ending with exactly one terminal event.
pub type RunStream = Pin<Box<dyn Stream<Item = RunEvent> + Send>>;

/// Result of a cheap, run-free readiness probe (used by `GET /cli/status` and
/// the startup log) — distinct from per-owner key/config state, which the route
/// composes on top.
pub struct RunnerHealth {
    /// `true` if the runner can be invoked (mock is always; the real runner
    /// requires `claude --version` to succeed).
    pub binary_ok: bool,
    /// Reported CLI version, when the probe could read it.
    pub version: Option<String>,
    /// Human-readable detail (e.g. why the probe failed).
    pub detail: Option<String>,
}

/// A failure to *start* a run (vs. a failure *during* one, which is an event).
#[derive(Debug, Error)]
pub enum RunError {
    /// The runner can't start a run at all (binary missing, disabled, …) → 503.
    #[error("{0}")]
    Unavailable(String),
}

/// What tools the spawned agent may use, beyond the always-allowed Baitler MCP
/// tools. Host `Bash`/`Write`/`Edit` are never enabled by this knob (that is the
/// separate `CLAUDE_CLI_ALLOW_HOST_TOOLS` config).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolScope {
    /// Only the Baitler MCP tools (`mcp__baitler`).
    KbOnly,
    /// The Baitler MCP tools plus read-only `Read` of the sandbox.
    KbPlusRead,
}

impl ToolScope {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "kb_only" => Some(Self::KbOnly),
            "kb_plus_read" => Some(Self::KbPlusRead),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::KbOnly => "kb_only",
            Self::KbPlusRead => "kb_plus_read",
        }
    }
}

/// Which model backend the `claude` CLI talks to for a run. Both run the same
/// binary; `Minimax` just points it at MiniMax's Anthropic-compatible endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentProvider {
    /// Anthropic (the CLI's own auth / a stored Anthropic key).
    ClaudeCode,
    /// MiniMax via its Anthropic-compatible endpoint (`MINIMAX_API_KEY`).
    Minimax,
}

impl AgentProvider {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "claude_code" => Some(Self::ClaudeCode),
            "minimax" => Some(Self::Minimax),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude_code",
            Self::Minimax => "minimax",
        }
    }
}

/// Everything needed to run one agent task. Built by the route from a validated
/// request; the API key (when present) lives only here and in the child env.
pub struct RunSpec {
    pub owner: String,
    pub run_id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub project_id: Option<String>,
    pub max_turns: u32,
    pub tool_scope: ToolScope,
    /// Which model backend to drive (Anthropic vs MiniMax's endpoint).
    pub provider: AgentProvider,
    /// Extra orienting context appended to the system prompt (e.g. which app page
    /// the user is on), via `--append-system-prompt`. `None` to omit.
    pub context: Option<String>,
    /// A user-granted, allow-listed local folder the run may **read** (added via
    /// `--add-dir`, enabling read-only `Read`/`Glob`/`Grep`). Already validated
    /// to a canonical path by the route. `None` = no host access.
    pub workspace_dir: Option<String>,
    /// Stable chat id (sanitized): turns sharing it run in the SAME persistent
    /// working dir so `--resume` finds the session. `None` = a one-shot run.
    pub conversation_id: Option<String>,
    /// Prior run's `session_id` to continue from (`--resume`).
    pub resume_session_id: Option<String>,
    /// API key/token for the child process (never logged, never in argv): the
    /// owner's Anthropic key for `ClaudeCode` (optional — else host auth), or the
    /// MiniMax subscription key for `Minimax` (required). `None` for the mock.
    pub api_key: Option<String>,
}

/// A spawnable agent backend.
#[async_trait]
pub trait AgentRunner: Send + Sync {
    /// Stable identifier (`"mock"` | `"claude-cli"`).
    fn kind(&self) -> &'static str;

    /// Whether the owner's stored Anthropic key should be injected into the
    /// child as `ANTHROPIC_API_KEY` **when present**. This is NOT a hard
    /// requirement: the real `claude` CLI authenticates via its own session
    /// (a `claude login` / subscription, or an `ANTHROPIC_API_KEY` already in
    /// the host env), so a run still proceeds without a Baitler-stored key —
    /// a stored key simply overrides the host's for that run.
    fn uses_key(&self) -> bool;

    /// Cheap, run-free readiness probe. The default reports OK (the mock needs
    /// no binary); the real runner overrides this to probe `claude --version`.
    async fn health(&self) -> RunnerHealth {
        RunnerHealth {
            binary_ok: true,
            version: None,
            detail: None,
        }
    }

    /// Start a run. `cancel` is notified (once) to abort in flight; the runner
    /// must then kill any subprocess and end the stream with a terminal event.
    async fn run(&self, spec: RunSpec, cancel: Arc<Notify>) -> Result<RunStream, RunError>;
}
