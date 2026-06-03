//! Events emitted during an agent run.
//!
//! A run is a stream of [`RunEvent`]s. Each event is forwarded to the client as
//! one SSE message (tagged by `type`); the terminal `Result`/`Error` event also
//! carries the fields persisted onto the `cli_run` row (session id, turns, cost).

use serde::Serialize;

/// A single event in an agent run's lifecycle.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    /// Session established. `session_id` is what a later run passes to `--resume`.
    Init {
        session_id: Option<String>,
        model: Option<String>,
    },
    /// A chunk of assistant prose.
    Assistant { text: String },
    /// The agent invoked a tool (summary is derived, never the raw arguments).
    ToolUse { name: String, summary: String },
    /// A tool returned.
    ToolResult { ok: bool, summary: String },
    /// Terminal: the run finished (success or a clean agent-reported error).
    Result {
        text: String,
        session_id: Option<String>,
        num_turns: i64,
        cost_usd: Option<f64>,
        is_error: bool,
    },
    /// Terminal: the run failed before producing a result (spawn/parse/timeout/
    /// cancel).
    Error { message: String },
}

impl RunEvent {
    /// Whether this is a terminal event (no further events follow).
    pub fn is_terminal(&self) -> bool {
        matches!(self, RunEvent::Result { .. } | RunEvent::Error { .. })
    }
}
