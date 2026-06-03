//! Data shapes for CLI runs (the `cli_run` row, its DTOs, and request bodies).

use serde::{Deserialize, Serialize};

/// A `cli_run` row as projected by the repository.
#[derive(Debug, Clone, Deserialize)]
pub struct CliRunRow {
    pub uuid: String,
    pub owner: String,
    pub prompt: String,
    pub model: Option<String>,
    pub tool_scope: String,
    pub status: String,
    pub session_id: Option<String>,
    pub conversation_id: Option<String>,
    pub num_turns: i64,
    pub cost_usd: Option<f64>,
    pub exit_code: Option<i64>,
    pub result_text: Option<String>,
    pub error: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

/// Full public representation of a run.
#[derive(Debug, Serialize)]
pub struct CliRunDto {
    pub id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub tool_scope: String,
    pub status: String,
    pub session_id: Option<String>,
    pub conversation_id: Option<String>,
    pub num_turns: i64,
    pub cost_usd: Option<f64>,
    pub exit_code: Option<i64>,
    pub result_text: Option<String>,
    pub error: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

impl From<CliRunRow> for CliRunDto {
    fn from(r: CliRunRow) -> Self {
        Self {
            id: r.uuid,
            prompt: r.prompt,
            model: r.model,
            tool_scope: r.tool_scope,
            status: r.status,
            session_id: r.session_id,
            conversation_id: r.conversation_id,
            num_turns: r.num_turns,
            cost_usd: r.cost_usd,
            exit_code: r.exit_code,
            result_text: r.result_text,
            error: r.error,
            project_id: r.project_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
            finished_at: r.finished_at,
        }
    }
}

/// Lightweight entry for the run list (omits `result_text` for payload size).
#[derive(Debug, Serialize)]
pub struct CliRunSummary {
    pub id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub tool_scope: String,
    pub status: String,
    pub session_id: Option<String>,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<CliRunRow> for CliRunSummary {
    fn from(r: CliRunRow) -> Self {
        Self {
            id: r.uuid,
            prompt: r.prompt,
            model: r.model,
            tool_scope: r.tool_scope,
            status: r.status,
            session_id: r.session_id,
            conversation_id: r.conversation_id,
            project_id: r.project_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// `POST /cli/runs` request body.
#[derive(Debug, Deserialize)]
pub struct CreateRunBody {
    pub prompt: String,
    /// Agent provider: `claude_code` (default) or `minimax`.
    #[serde(default)]
    pub provider: Option<String>,
    /// Orienting context appended to the system prompt (e.g. the current page).
    #[serde(default)]
    pub context: Option<String>,
    /// A local folder to grant the run read-only access to (must resolve within a
    /// server-configured allow-listed root).
    #[serde(default)]
    pub workspace_dir: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub max_turns: Option<u32>,
    /// `kb_only` (default) or `kb_plus_read`.
    #[serde(default)]
    pub tool_scope: Option<String>,
    /// Prior run's `session_id` to continue from.
    #[serde(default)]
    pub resume_session_id: Option<String>,
    /// Stable id for a multi-turn chat: all turns sharing it run in the same
    /// working directory so `--resume` can find the session.
    #[serde(default)]
    pub conversation_id: Option<String>,
}
