//! An offline agent runner that emits a deterministic, scripted run. No
//! subprocess, no network — so `/cli/runs` works end-to-end in dev/CI without a
//! `claude` binary or any egress. Selected when `CLAUDE_BIN == "mock"`.
//!
//! Mirrors [`crate::llm::mock::MockProvider`]'s role for the LLM layer: it is the
//! asserted path, while the real [`ClaudeCliRunner`](super::claude::ClaudeCliRunner)
//! is compiled but unexercised here.

use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use async_trait::async_trait;
use tokio::sync::Notify;

use super::events::RunEvent;
use super::runner::{AgentRunner, RunError, RunSpec, RunStream};

#[derive(Default)]
pub struct MockRunner;

/// Emit `event` after a short delay, racing against cancellation. Returns `false`
/// if cancelled (the caller should stop and emit a terminal error).
macro_rules! step {
    ($cancel:expr) => {{
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(10)) => true,
            _ = $cancel.notified() => false,
        }
    }};
}

#[async_trait]
impl AgentRunner for MockRunner {
    fn kind(&self) -> &'static str {
        "mock"
    }

    fn uses_key(&self) -> bool {
        false
    }

    async fn run(&self, spec: RunSpec, cancel: Arc<Notify>) -> Result<RunStream, RunError> {
        // Deterministic session id derived from the run id, so `--resume`-style
        // round-trips are testable without real state.
        let session_id = spec
            .resume_session_id
            .clone()
            .unwrap_or_else(|| format!("mock-{}", spec.run_id));
        let model = spec
            .model
            .clone()
            .unwrap_or_else(|| "mock-model".to_string());
        let prompt_echo: String = spec.prompt.chars().take(120).collect();

        let s = stream! {
            yield RunEvent::Init {
                session_id: Some(session_id.clone()),
                model: Some(model.clone()),
            };

            if !step!(cancel) { yield RunEvent::Error { message: "run cancelled".into() }; return; }
            yield RunEvent::Assistant {
                text: format!("Working on your request: \"{prompt_echo}\"."),
            };

            if !step!(cancel) { yield RunEvent::Error { message: "run cancelled".into() }; return; }
            yield RunEvent::ToolUse {
                name: "mcp__baitler__ideas_create".into(),
                summary: "create a draft idea in the knowledge base".into(),
            };

            if !step!(cancel) { yield RunEvent::Error { message: "run cancelled".into() }; return; }
            yield RunEvent::ToolResult {
                ok: true,
                summary: "created a draft idea (pending review)".into(),
            };

            if !step!(cancel) { yield RunEvent::Error { message: "run cancelled".into() }; return; }
            yield RunEvent::Result {
                text: format!(
                    "Mock run complete — no real agent was invoked. I captured a draft idea for: \
                     \"{prompt_echo}\". Set CLAUDE_CLI_ENABLED and CLAUDE_BIN to run the real \
                     Claude Code CLI."
                ),
                session_id: Some(session_id),
                num_turns: 2,
                cost_usd: Some(0.0),
                is_error: false,
            };
        };

        Ok(Box::pin(s))
    }
}
