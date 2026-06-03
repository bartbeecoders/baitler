//! Owner-scoped HTTP surface for CLI runs (Phase 13.5).
//!
//! `POST /cli/runs` starts a run and streams [`RunEvent`]s over SSE (reusing the
//! Phase 6 `ai/chat` pattern), persisting the terminal outcome onto the row;
//! `GET /cli/runs` lists, `GET /cli/runs/{id}` fetches, and
//! `POST /cli/runs/{id}/cancel` aborts an in-flight run. The runner is **off by
//! default** (503) and serialised to **one active run per owner** (409).

use std::convert::Infallible;

use async_stream::stream;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::crypto;
use crate::error::{AppError, AppResult};
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::events::RunEvent;
use super::model::{CliRunDto, CliRunSummary, CreateRunBody};
use super::repo::{self, RunOutcome};
use super::runner::{AgentProvider, RunError, RunSpec, ToolScope};

/// Provider id whose stored key supplies `ANTHROPIC_API_KEY` to the child.
const ANTHROPIC_PROVIDER: &str = "anthropic";

/// Reduce a client-supplied conversation id to a path-safe token (it becomes part
/// of a directory name). Keeps ASCII alphanumerics + `-`/`_`, caps length, and
/// returns `None` if nothing usable remains (→ a one-shot run).
fn sanitize_conversation_id(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(64)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}
const MAX_PROMPT_LEN: usize = 50_000;
const MAX_CONTEXT_LEN: usize = 4_000;
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 200;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cli/status", get(status))
        .route("/cli/runs", get(list).post(create))
        .route("/cli/runs/{id}", get(get_one))
        .route("/cli/runs/{id}/cancel", post(cancel))
}

/// Run-free readiness for the Agent page: composes config (`enabled`), the
/// runner's binary probe, the owner's key signals, and per-provider availability
/// — so a misconfigured `claude`/MiniMax is diagnosable without a failed run.
#[derive(Debug, Serialize)]
struct CliStatus {
    enabled: bool,
    kind: String,
    binary_ok: bool,
    version: Option<String>,
    has_stored_key: bool,
    host_key_env: bool,
    ready: bool,
    message: String,
    /// Selectable agent providers and whether each can run right now.
    providers: Vec<ProviderStatus>,
    /// Allow-listed local roots a run may be granted read access to (empty =
    /// the local-folder import feature is off).
    workspace_roots: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProviderStatus {
    id: String,
    label: String,
    available: bool,
    detail: String,
}

async fn status(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
) -> AppResult<Json<CliStatus>> {
    let cli = &state.config.cli;
    let runner = &state.cli_runner;
    let kind = runner.kind().to_string();
    let is_mock = runner.kind() == "mock";
    let has_stored_key = crate::ai::repo::get_ciphertext(&state.db, &owner, ANTHROPIC_PROVIDER)
        .await?
        .is_some();
    let host_key_env = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .is_some_and(|v| !v.trim().is_empty());
    let minimax_configured = cli.minimax_api_key.is_some();

    // Probe the binary (skipped for disabled/mock, which can't or needn't run it).
    let (binary_ok, version, binary_detail) = if !cli.enabled {
        (false, None, None)
    } else if is_mock {
        (true, None, None)
    } else {
        let h = runner.health().await;
        (h.binary_ok, h.version, h.detail)
    };

    // Overall readiness + the binary/auth hint (drives the global banner).
    let (ready, message) = if !cli.enabled {
        (
            false,
            "The agent runner is disabled. Set CLAUDE_CLI_ENABLED=true on the server.".to_string(),
        )
    } else if !binary_ok {
        (
            false,
            format!(
                "`claude` could not be run on the server ({}). Check CLAUDE_BIN and the server PATH.",
                binary_detail.as_deref().unwrap_or("not found")
            ),
        )
    } else if is_mock {
        (
            true,
            "Using the offline mock runner (CLAUDE_BIN=mock).".to_string(),
        )
    } else if has_stored_key {
        (
            true,
            "Ready — Claude Code will use the Anthropic key from your AI settings.".to_string(),
        )
    } else if host_key_env {
        (
            true,
            "Ready — Claude Code will use the server's ANTHROPIC_API_KEY.".to_string(),
        )
    } else {
        (
            true,
            "claude is installed but no Anthropic key was detected — Claude Code relies on a \
             `claude login` session on the server. (MiniMax uses MINIMAX_API_KEY instead.)"
                .to_string(),
        )
    };

    // Per-provider availability (both drive the same `claude` binary).
    let claude_detail = if !cli.enabled {
        "The agent runner is disabled."
    } else if !binary_ok {
        "claude is not runnable on the server."
    } else if is_mock {
        "Offline mock runner."
    } else if has_stored_key || host_key_env {
        "Anthropic (key configured)."
    } else {
        "Anthropic (uses a `claude login` session if present)."
    };
    let minimax_available = cli.enabled && binary_ok && (is_mock || minimax_configured);
    let minimax_detail = if !cli.enabled {
        "The agent runner is disabled.".to_string()
    } else if !binary_ok {
        "claude is not runnable on the server.".to_string()
    } else if !is_mock && !minimax_configured {
        "Not configured — set MINIMAX_API_KEY on the server.".to_string()
    } else {
        format!(
            "MiniMax via its Anthropic-compatible endpoint ({}).",
            cli.minimax_model
        )
    };

    let providers = vec![
        ProviderStatus {
            id: "claude_code".into(),
            label: "Claude Code".into(),
            available: cli.enabled && binary_ok,
            detail: claude_detail.to_string(),
        },
        ProviderStatus {
            id: "minimax".into(),
            label: cli.minimax_model.clone(),
            available: minimax_available,
            detail: minimax_detail,
        },
    ];

    Ok(Json(CliStatus {
        enabled: cli.enabled,
        kind,
        binary_ok,
        version,
        has_stored_key,
        host_key_env,
        ready,
        message,
        providers,
        workspace_roots: cli.workspace_roots.clone(),
    }))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[derive(Debug, Serialize)]
struct RunListResponse {
    runs: Vec<CliRunSummary>,
}

async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<RunListResponse>> {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = q.offset.unwrap_or(0);
    let runs = repo::list_runs(
        &state.db,
        &owner,
        q.project.as_deref().filter(|s| !s.is_empty()),
        q.status.as_deref().filter(|s| !s.is_empty()),
        limit,
        offset,
    )
    .await?;
    Ok(Json(RunListResponse {
        runs: runs.into_iter().map(Into::into).collect(),
    }))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<CliRunDto>> {
    let run = repo::get_run(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(run.into()))
}

async fn cancel(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    // Must exist and be owned before we touch the registry.
    repo::get_run(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    if state.cli_runs.cancel(&id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::Conflict("run is not active".into()))
    }
}

async fn create(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreateRunBody>,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let cli = &state.config.cli;
    if !cli.enabled {
        return Err(AppError::Unavailable(
            "the Claude Code CLI runner is disabled (set CLAUDE_CLI_ENABLED)".into(),
        ));
    }

    // Validate the request.
    let prompt = body.prompt.trim();
    if prompt.is_empty() {
        return Err(AppError::BadRequest("prompt must not be empty".into()));
    }
    if prompt.len() > MAX_PROMPT_LEN {
        return Err(AppError::BadRequest("prompt is too long".into()));
    }
    let tool_scope = match body.tool_scope.as_deref() {
        None | Some("") => ToolScope::KbOnly,
        Some(s) => ToolScope::parse(s).ok_or_else(|| {
            AppError::BadRequest("invalid tool_scope (expected kb_only or kb_plus_read)".into())
        })?,
    };
    let provider = match body.provider.as_deref() {
        None | Some("") => AgentProvider::ClaudeCode,
        Some(s) => AgentProvider::parse(s).ok_or_else(|| {
            AppError::BadRequest("invalid provider (expected claude_code or minimax)".into())
        })?,
    };
    let max_turns = body
        .max_turns
        .unwrap_or(cli.max_turns)
        .clamp(1, cli.max_turns);
    let requested_model = body.model.clone().filter(|s| !s.trim().is_empty());
    let project_id = body.project_id.clone().filter(|s| !s.trim().is_empty());
    let resume = body
        .resume_session_id
        .clone()
        .filter(|s| !s.trim().is_empty());
    // Sanitize to a path-safe token — it becomes part of a directory name.
    let conversation_id = body
        .conversation_id
        .as_deref()
        .and_then(sanitize_conversation_id);
    let context = body
        .context
        .clone()
        .map(|c| c.chars().take(MAX_CONTEXT_LEN).collect::<String>())
        .filter(|c| !c.trim().is_empty());

    // A requested local folder must resolve within an allow-listed root; the
    // canonical path is what we hand the runner.
    let workspace_dir = match body
        .workspace_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(path) => Some(
            crate::cli::validate_workspace(&cli.workspace_roots, path)
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(AppError::BadRequest)?,
        ),
        None => None,
    };

    let runner = state.cli_runner.clone();

    // Effective model: the request's, else a provider-specific default. (Applied
    // regardless of runner, so the row records what would run.)
    let model = requested_model.or_else(|| match provider {
        AgentProvider::Minimax => Some(cli.minimax_model.clone()),
        AgentProvider::ClaudeCode => cli.default_model.clone(),
    });

    // Per-provider key — flows ONLY through the child env (never argv/logs), and
    // only for runners that use one (the mock needs none).
    let api_key: Option<String> = if !runner.uses_key() {
        None
    } else {
        match provider {
            // Optionally inject the owner's stored Anthropic key to override the
            // host's. Its ABSENCE is fine — `claude` uses its own auth (a
            // `claude login` session, or a host `ANTHROPIC_API_KEY`) — so we never
            // 503 here; an auth failure surfaces as a run error event instead.
            AgentProvider::ClaudeCode => {
                match crate::ai::repo::get_ciphertext(&state.db, &owner, ANTHROPIC_PROVIDER).await?
                {
                    Some(ciphertext) => Some(
                        crypto::decrypt(&state.config.secret_key, &ciphertext)
                            .map_err(|e| AppError::Internal(Box::new(e)))?,
                    ),
                    None => None,
                }
            }
            // MiniMax drives the same CLI against its Anthropic-compatible
            // endpoint; the subscription key is required (clear 503 if unset).
            AgentProvider::Minimax => Some(cli.minimax_api_key.clone().ok_or_else(|| {
                AppError::Unavailable(
                    "MiniMax is not configured — set MINIMAX_API_KEY on the server.".into(),
                )
            })?),
        }
    };

    // One active run per owner.
    let run_id = repo::new_run_id();
    let cancel_signal = state.cli_runs.begin(&owner, &run_id).ok_or_else(|| {
        AppError::Conflict(
            "a run is already in progress; wait for it to finish or cancel it".into(),
        )
    })?;

    // Persist the row (running). Release the slot on failure.
    if let Err(e) = repo::create_run(
        &state.db,
        &owner,
        &run_id,
        prompt,
        model.as_deref(),
        tool_scope.as_str(),
        project_id.as_deref(),
        conversation_id.as_deref(),
    )
    .await
    {
        state.cli_runs.finish(&owner, &run_id);
        return Err(e);
    }

    let spec = RunSpec {
        owner: owner.clone(),
        run_id: run_id.clone(),
        prompt: prompt.to_string(),
        model,
        project_id,
        max_turns,
        tool_scope,
        provider,
        context,
        workspace_dir,
        conversation_id,
        resume_session_id: resume,
        api_key,
    };

    // Spawn-time failures surface as a 503 before any stream begins.
    let events = match runner.run(spec, cancel_signal).await {
        Ok(s) => s,
        Err(RunError::Unavailable(msg)) => {
            let _ = repo::finalize_run(
                &state.db,
                &owner,
                &run_id,
                &RunOutcome {
                    status: "failed".into(),
                    error: Some(msg.clone()),
                    ..Default::default()
                },
            )
            .await;
            state.cli_runs.finish(&owner, &run_id);
            return Err(AppError::Unavailable(msg));
        }
    };

    let sse = run_sse(state.clone(), owner, run_id, events);
    Ok(Sse::new(sse).keep_alive(KeepAlive::default()))
}

/// Releases the per-owner active slot when the stream completes *or is dropped*
/// (e.g. the client disconnects), so a slot can't leak. The DB row's terminal
/// state is written inside the stream before this runs.
struct FinishGuard {
    state: AppState,
    owner: String,
    run_id: String,
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.state.cli_runs.finish(&self.owner, &self.run_id);
    }
}

/// Forward run events as SSE, then persist the terminal outcome and free the
/// owner's slot.
fn run_sse(
    state: AppState,
    owner: String,
    run_id: String,
    events: super::runner::RunStream,
) -> impl Stream<Item = Result<Event, Infallible>> {
    stream! {
        let _guard = FinishGuard {
            state: state.clone(),
            owner: owner.clone(),
            run_id: run_id.clone(),
        };
        let mut events = events;
        let mut outcome: Option<RunOutcome> = None;

        // Leading event: hand the client the run id so it can cancel this run
        // (the cancel endpoint needs the id) and link the live stream to history.
        yield Ok::<_, Infallible>(Event::default()
            .json_data(json!({ "type": "run", "id": run_id }))
            .unwrap());

        while let Some(ev) = events.next().await {
            if let Some(o) = outcome_from(&ev) {
                outcome = Some(o);
            }
            yield Ok::<_, Infallible>(Event::default().json_data(&ev).unwrap());
        }

        // Resolve the final status: an explicit cancel wins over the terminal
        // event's own classification.
        let mut final_outcome = outcome.unwrap_or_else(|| RunOutcome {
            status: "failed".into(),
            error: Some("the run ended without a result".into()),
            ..Default::default()
        });
        if state.cli_runs.was_cancelled(&run_id) {
            final_outcome.status = "cancelled".into();
        }
        let status = final_outcome.status.clone();

        let _ = repo::finalize_run(&state.db, &owner, &run_id, &final_outcome).await;
        yield Ok(Event::default()
            .json_data(json!({ "type": "done", "status": status }))
            .unwrap());
    }
}

/// Map a terminal [`RunEvent`] to the row's terminal fields (non-terminal events
/// return `None`).
fn outcome_from(ev: &RunEvent) -> Option<RunOutcome> {
    match ev {
        RunEvent::Result {
            text,
            session_id,
            num_turns,
            cost_usd,
            is_error,
        } => Some(RunOutcome {
            status: if *is_error { "failed" } else { "succeeded" }.into(),
            session_id: session_id.clone(),
            num_turns: *num_turns,
            cost_usd: *cost_usd,
            exit_code: None,
            result_text: Some(text.clone()),
            error: is_error.then(|| text.clone()),
        }),
        RunEvent::Error { message } => Some(RunOutcome {
            status: "failed".into(),
            error: Some(message.clone()),
            ..Default::default()
        }),
        _ => None,
    }
}
