//! The real agent runner: spawns the `claude` CLI headless and parses its
//! `--output-format stream-json` output (Phase 13.2/13.3).
//!
//! Security posture (see the Phase 13 plan):
//! - **No shell.** The argv is built as a vector and the prompt is a discrete
//!   element — there is no string for an injection to live in.
//! - **Secret in the child env only.** `ANTHROPIC_API_KEY` is set on the child
//!   process, never in argv (which is world-readable via `ps`) or logs.
//! - **Filesystem sandbox.** Each run gets an ephemeral cwd under the configured
//!   workdir, removed when the stream ends; `--add-dir` is limited to it.
//! - **Tool scope.** Only the Baitler MCP tools are pre-approved; host
//!   `Bash`/`Write`/`Edit` are disallowed unless explicitly opted in, and
//!   `--dangerously-skip-permissions` is never passed.
//! - **MCP loopback.** A generated `--mcp-config` (+ `--strict-mcp-config`)
//!   points the agent back at Baitler's own `/mcp` with `X-Baitler-Agent`, so
//!   its writes are draft-gated and attributed like any other agent.
//!
//! OS-level isolation (container/cgroup/seccomp, network egress allowlist, true
//! process-group kill) is a Phase 13.D / Phase 10 hardening item; here we rely on
//! the sandbox dir, tool denial, a wall-clock timeout, and `kill_on_drop`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;

use crate::config::CliConfig;

use super::events::RunEvent;
use super::runner::{AgentProvider, AgentRunner, RunError, RunSpec, RunStream, ToolScope};
use super::AGENT_LABEL;

/// Allowed tool token that approves *all* tools from the Baitler MCP server.
const BAITLER_MCP_TOOLS: &str = "mcp__baitler";

/// Operating constraints appended to every spawned run's system prompt, so the
/// agent knows what it actually can and cannot do (Phase 13's lockdown is
/// invisible from inside the session): the run is headless — a tool-permission
/// denial is FINAL, there is no prompt anyone can approve — and the way to get
/// disk access is to have the user attach the folder, not to retry.
const RUN_PREAMBLE: &str = "You are Baitler's butler agent, spawned non-interactively \
from the Baitler web portal. Tool-permission prompts can NEVER be approved mid-run: \
if a tool call is denied, that access does not exist for this run — do not retry it \
and do not ask the user to approve anything in a terminal. Your local-disk access is \
exactly the folder attached to this conversation (if any); the Baitler `workspace_*` \
MCP tools are reserved for external clients and always refuse spawned runs. If the \
user names a local path you cannot read — even one inside the server's allow-listed \
roots — ask them to attach that folder via \"Attach a folder\" on the Baitler home \
and re-send their request. For knowledge-base work, prefer the Baitler MCP tools \
(knowledge_search to find things; ideas/documents/pages tools to save results — \
agent writes land as drafts for the user's review).";

pub struct ClaudeCliRunner {
    cfg: CliConfig,
    /// Bearer token for the loopback `/mcp` (mirrors `MCP_AUTH_TOKEN`), if set.
    mcp_auth_token: Option<String>,
    /// URL the spawned agent's MCP config points back at.
    loopback_url: String,
}

impl ClaudeCliRunner {
    pub fn new(cfg: CliConfig, mcp_auth_token: Option<String>, loopback_url: String) -> Self {
        Self {
            cfg,
            mcp_auth_token,
            loopback_url,
        }
    }
}

#[async_trait]
impl AgentRunner for ClaudeCliRunner {
    fn kind(&self) -> &'static str {
        "claude-cli"
    }

    fn uses_key(&self) -> bool {
        true
    }

    async fn health(&self) -> super::runner::RunnerHealth {
        use super::runner::RunnerHealth;
        let probe = Command::new(&self.cfg.bin)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output();
        match tokio::time::timeout(Duration::from_secs(5), probe).await {
            Ok(Ok(out)) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                RunnerHealth {
                    binary_ok: true,
                    version: (!version.is_empty()).then_some(version),
                    detail: None,
                }
            }
            Ok(Ok(out)) => RunnerHealth {
                binary_ok: false,
                version: None,
                detail: Some(format!(
                    "`{} --version` exited with {}",
                    self.cfg.bin, out.status
                )),
            },
            Ok(Err(e)) => RunnerHealth {
                binary_ok: false,
                version: None,
                detail: Some(format!("could not run `{}`: {e}", self.cfg.bin)),
            },
            Err(_) => RunnerHealth {
                binary_ok: false,
                version: None,
                detail: Some(format!("`{} --version` timed out", self.cfg.bin)),
            },
        }
    }

    async fn run(&self, spec: RunSpec, cancel: Arc<Notify>) -> Result<RunStream, RunError> {
        // 1. Working directory. A conversation reuses a STABLE `conv-<id>` dir
        // across its turns, because `claude` keys resumable sessions to the cwd —
        // a per-run throwaway dir would make `--resume` find nothing. A one-shot
        // run uses a throwaway `run-<id>` dir. Resolve to an ABSOLUTE path: the
        // child runs with `current_dir(sandbox)`, so a relative
        // `--mcp-config`/`--add-dir` would otherwise re-resolve against that cwd.
        let persistent = spec
            .conversation_id
            .as_deref()
            .is_some_and(|c| !c.is_empty());
        let sandbox = PathBuf::from(&self.cfg.workdir).join(conv_dirname(&spec));
        tokio::fs::create_dir_all(&sandbox)
            .await
            .map_err(|e| RunError::Unavailable(format!("could not create run sandbox: {e}")))?;
        let sandbox = tokio::fs::canonicalize(&sandbox).await.unwrap_or(sandbox);

        // 2. Loopback MCP config, written 0600 inside the sandbox. Rewritten on
        // every turn so the X-Baitler-Run header always names the CURRENT run.
        let mcp_path = sandbox.join("mcp.json");
        let mcp_json = build_mcp_config(
            &self.loopback_url,
            self.mcp_auth_token.as_deref(),
            &spec.run_id,
        );
        if let Err(e) = tokio::fs::write(&mcp_path, mcp_json.to_string()).await {
            let _ = tokio::fs::remove_dir_all(&sandbox).await;
            return Err(RunError::Unavailable(format!(
                "could not write MCP loopback config: {e}"
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                tokio::fs::set_permissions(&mcp_path, std::fs::Permissions::from_mode(0o600)).await;
        }

        // 3. Build argv (no shell, no secrets) and spawn.
        let argv = build_argv(&self.cfg, &spec, &sandbox, &mcp_path);
        let mut cmd = Command::new(&self.cfg.bin);
        cmd.args(&argv)
            .current_dir(&sandbox)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            // Capture stderr so a crash/usage/auth error is reportable to the
            // user (rather than a bare "exited without a result").
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Apply provider env (key/endpoint) into the CHILD env only — never argv
        // or logs. For ClaudeCode with no stored key, nothing is set and the child
        // inherits the host's `claude` auth (a `claude login` session, or a host
        // `ANTHROPIC_API_KEY`); for Minimax we point the same CLI at MiniMax's
        // Anthropic-compatible endpoint.
        for (k, v) in provider_env(
            spec.provider,
            &self.cfg.minimax_base_url,
            spec.model.as_deref(),
            spec.api_key.as_deref(),
        ) {
            cmd.env(k, v);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&sandbox).await;
                return Err(RunError::Unavailable(format!(
                    "failed to start the Claude Code CLI (`{}`): {e}",
                    self.cfg.bin
                )));
            }
        };

        let stdout = child
            .stdout
            .take()
            .expect("child stdout was piped above, so it is present");
        let mut lines = BufReader::new(stdout).lines();
        // Drain stderr concurrently into a capped buffer so reporting it can't
        // block on a slow/never-closing pipe.
        let stderr_task = child
            .stderr
            .take()
            .map(|err| tokio::spawn(drain_capped(err)));
        let timeout = self.cfg.timeout;

        let s = stream! {
            let deadline = tokio::time::sleep(timeout);
            tokio::pin!(deadline);
            let mut terminal_emitted = false;

            loop {
                tokio::select! {
                    biased;
                    _ = cancel.notified() => {
                        let _ = child.start_kill();
                        yield RunEvent::Error { message: "run cancelled".into() };
                        terminal_emitted = true;
                        break;
                    }
                    _ = &mut deadline => {
                        let _ = child.start_kill();
                        yield RunEvent::Error { message: "run timed out".into() };
                        terminal_emitted = true;
                        break;
                    }
                    next = lines.next_line() => match next {
                        Ok(Some(line)) => {
                            for ev in parse_line(&line) {
                                if ev.is_terminal() { terminal_emitted = true; }
                                yield ev;
                            }
                        }
                        Ok(None) => break, // EOF — process closed stdout
                        Err(e) => {
                            yield RunEvent::Error {
                                message: format!("failed reading agent output: {e}"),
                            };
                            terminal_emitted = true;
                            break;
                        }
                    }
                }
            }

            let status = child.wait().await;
            if !terminal_emitted {
                // No `result` event — the process errored before completing. Surface
                // the exit code + captured stderr so the failure is actionable.
                let code = status.ok().and_then(|s| s.code());
                let stderr = match stderr_task {
                    Some(handle) => handle.await.unwrap_or_default(),
                    None => String::new(),
                };
                let mut message = match code {
                    Some(c) => format!("the agent exited (code {c}) without producing a result"),
                    None => "the agent exited without producing a result".to_string(),
                };
                if !stderr.is_empty() {
                    message.push_str(&format!(": {}", truncate(&stderr, 600)));
                }
                yield RunEvent::Error { message };
            }
            // Tear down a one-shot sandbox; KEEP a conversation dir so its cwd
            // stays stable for the next turn's `--resume` (it holds only mcp.json).
            if !persistent {
                let _ = tokio::fs::remove_dir_all(&sandbox).await;
            }
        };

        Ok(Box::pin(s))
    }
}

/// Read an async byte source to end-of-stream, keeping at most the first
/// `STDERR_CAP` bytes (so a chatty/never-ending process can't grow this without
/// bound), and return it as a trimmed lossy-UTF-8 string.
async fn drain_capped<R>(reader: R) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    const STDERR_CAP: usize = 8 * 1024;
    use tokio::io::AsyncReadExt;
    let mut reader = BufReader::new(reader);
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() < STDERR_CAP {
                    let room = STDERR_CAP - buf.len();
                    buf.extend_from_slice(&chunk[..n.min(room)]);
                }
            }
        }
    }
    String::from_utf8_lossy(&buf).trim().to_string()
}

/// Environment to inject for a run's agent provider (child env only, never argv).
/// Pure + `pub(crate)` so it's unit-testable. For `ClaudeCode` a key is optional
/// (absent → inherit host auth); for `Minimax` the same `claude` CLI is pointed at
/// MiniMax's Anthropic-compatible endpoint via `ANTHROPIC_BASE_URL` + the
/// `ANTHROPIC_AUTH_TOKEN` subscription key (per MiniMax's docs).
pub(crate) fn provider_env(
    provider: AgentProvider,
    minimax_base_url: &str,
    model: Option<&str>,
    key: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    match provider {
        AgentProvider::ClaudeCode => {
            if let Some(k) = key {
                env.push(("ANTHROPIC_API_KEY", k.to_string()));
            }
        }
        AgentProvider::Minimax => {
            env.push(("ANTHROPIC_BASE_URL", minimax_base_url.to_string()));
            if let Some(k) = key {
                env.push(("ANTHROPIC_AUTH_TOKEN", k.to_string()));
                env.push(("ANTHROPIC_API_KEY", k.to_string()));
            }
            // Map EVERY model slot to MiniMax. Claude Code otherwise issues
            // background/"small-fast" calls with a *Claude* model name (e.g.
            // claude-…-haiku) that MiniMax's gateway doesn't have → those 404 and
            // can stall tool use, so MCP calls appear not to work. Pointing all
            // slots at the MiniMax model keeps every request on the gateway.
            if let Some(m) = model {
                for var in [
                    "ANTHROPIC_MODEL",
                    "ANTHROPIC_SMALL_FAST_MODEL",
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                    "ANTHROPIC_DEFAULT_SONNET_MODEL",
                    "ANTHROPIC_DEFAULT_OPUS_MODEL",
                ] {
                    env.push((var, m.to_string()));
                }
            }
        }
    }
    env
}

/// Working-directory name for a run: a stable `conv-<id>` for a conversation
/// (so `--resume` finds the session), else a throwaway `run-<run_id>`. The
/// `conversation_id` is already sanitized by the route to a path-safe token.
pub(crate) fn conv_dirname(spec: &RunSpec) -> String {
    match spec.conversation_id.as_deref() {
        Some(c) if !c.is_empty() => format!("conv-{c}"),
        _ => format!("run-{}", spec.run_id),
    }
}

/// Build the `claude` argv. Pure + `pub(crate)` so the security-critical flag set
/// is unit-testable (no `--dangerously-skip-permissions`, host tools denied, the
/// API key absent, the prompt a discrete element).
pub(crate) fn build_argv(
    cfg: &CliConfig,
    spec: &RunSpec,
    sandbox: &Path,
    mcp_config_path: &Path,
) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-p".into(),
        spec.prompt.clone(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--permission-mode".into(),
        "default".into(),
        "--max-turns".into(),
        spec.max_turns.to_string(),
        "--strict-mcp-config".into(),
        "--mcp-config".into(),
        mcp_config_path.display().to_string(),
        "--add-dir".into(),
        sandbox.display().to_string(),
    ];

    // A user-granted local folder is added as a read-only working dir.
    let workspace = spec.workspace_dir.as_deref().filter(|w| !w.is_empty());
    if let Some(ws) = workspace {
        a.push("--add-dir".into());
        a.push(ws.to_string());
    }

    if let Some(model) = spec.model.clone().or_else(|| cfg.default_model.clone()) {
        a.push("--model".into());
        a.push(model);
    }

    // Context appended to the system prompt. Always starts with the operating
    // constraints — the run is non-interactive and its disk access is exactly
    // the attached folder — so the agent never stalls telling the user to
    // approve a permission prompt nobody can answer (and knows the recovery is
    // "attach the folder"). Then the folder grant (so an unqualified "list the
    // files" resolves to it rather than the empty sandbox cwd), then the
    // caller's note (e.g. the current app page).
    let mut context_parts: Vec<String> = vec![RUN_PREAMBLE.to_string()];
    match workspace {
        Some(ws) => context_parts.push(format!(
            "The user attached the local folder {ws} to this conversation; it is your \
             ONLY local-disk access, read-only via the Read, Glob, and Grep tools. \
             When they refer to \"the folder\", \"these files\", or ask about files \
             without naming a location, they mean that folder."
        )),
        None => context_parts.push(
            "No local folder is attached to this conversation, so you have NO \
             local-disk access this run."
                .to_string(),
        ),
    }
    if let Some(context) = spec.context.as_deref().filter(|c| !c.trim().is_empty()) {
        context_parts.push(context.to_string());
    }
    a.push("--append-system-prompt".into());
    a.push(context_parts.join("\n\n"));

    // Pre-approve exactly the Baitler MCP tools (plus read-only file tools when
    // asked for / a folder is granted), so the non-interactive run never blocks on
    // a permission prompt. A granted workspace also gets Glob/Grep so the agent can
    // find files in it; host Bash/Write/Edit stay denied unless allow_host_tools.
    let mut allowed = vec![BAITLER_MCP_TOOLS.to_string()];
    let has_workspace = workspace.is_some();
    if spec.tool_scope == ToolScope::KbPlusRead || has_workspace {
        allowed.push("Read".into());
    }
    if has_workspace {
        allowed.push("Glob".into());
        allowed.push("Grep".into());
    }
    a.push("--allowedTools".into());
    a.push(allowed.join(","));

    // Host filesystem/shell tools are denied unless explicitly opted in.
    if !cfg.allow_host_tools {
        a.push("--disallowedTools".into());
        a.push("Bash,Write,Edit".into());
    }

    if let Some(session) = &spec.resume_session_id {
        a.push("--resume".into());
        a.push(session.clone());
    }

    a
}

/// Build the loopback MCP config the spawned agent uses (Phase 13.3). Points at
/// Baitler's own `/mcp`, tags every call with `X-Baitler-Agent` for attribution
/// and `X-Baitler-Run` for run↔activity correlation (the butler report), and
/// forwards the optional bearer token.
pub(crate) fn build_mcp_config(
    loopback_url: &str,
    auth_token: Option<&str>,
    run_id: &str,
) -> Value {
    let mut headers = serde_json::Map::new();
    headers.insert("X-Baitler-Agent".to_string(), json!(AGENT_LABEL));
    headers.insert("X-Baitler-Run".to_string(), json!(run_id));
    if let Some(token) = auth_token {
        headers.insert(
            "Authorization".to_string(),
            json!(format!("Bearer {token}")),
        );
    }
    json!({
        "mcpServers": {
            "baitler": {
                "type": "http",
                "url": loopback_url,
                "headers": Value::Object(headers),
            }
        }
    })
}

/// Parse one line of `stream-json` output into zero or more [`RunEvent`]s.
/// Tolerant of unknown event types and missing fields (never panics) so a CLI
/// version bump can't crash a run. Pure + `pub(crate)` for unit testing.
pub(crate) fn parse_line(line: &str) -> Vec<RunEvent> {
    let line = line.trim();
    if line.is_empty() {
        return vec![];
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    match v.get("type").and_then(Value::as_str).unwrap_or("") {
        "system" if v.get("subtype").and_then(Value::as_str) == Some("init") => {
            vec![RunEvent::Init {
                session_id: str_field(&v, "session_id"),
                model: str_field(&v, "model"),
            }]
        }
        "assistant" => {
            let mut out = Vec::new();
            if let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) {
                for block in blocks {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                if !text.is_empty() {
                                    out.push(RunEvent::Assistant {
                                        text: text.to_string(),
                                    });
                                }
                            }
                        }
                        Some("tool_use") => {
                            let name = block
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("tool")
                                .to_string();
                            out.push(RunEvent::ToolUse {
                                name,
                                summary: summarize_input(block.get("input")),
                            });
                        }
                        _ => {}
                    }
                }
            }
            out
        }
        "user" => {
            let mut out = Vec::new();
            if let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) {
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) == Some("tool_result") {
                        let ok = !block
                            .get("is_error")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        out.push(RunEvent::ToolResult {
                            ok,
                            summary: summarize_content(block.get("content")),
                        });
                    }
                }
            }
            out
        }
        "result" => {
            let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false)
                || v.get("subtype")
                    .and_then(Value::as_str)
                    .map(|s| s != "success")
                    .unwrap_or(false);
            vec![RunEvent::Result {
                text: v
                    .get("result")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                session_id: str_field(&v, "session_id"),
                num_turns: v.get("num_turns").and_then(Value::as_i64).unwrap_or(0),
                cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
                is_error,
            }]
        }
        _ => vec![],
    }
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Short, argument-free summary of a tool's input (never the raw arguments).
fn summarize_input(input: Option<&Value>) -> String {
    match input {
        Some(v) => truncate(&v.to_string(), 200),
        None => String::new(),
    }
}

/// Summarize a tool result, which may be a string or an array of content blocks.
fn summarize_content(content: Option<&Value>) -> String {
    let text = match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    truncate(&text, 200)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(api_key: Option<&str>, scope: ToolScope, resume: Option<&str>) -> RunSpec {
        RunSpec {
            owner: "dev".into(),
            run_id: "run-1".into(),
            prompt: "organise my inbox; rm -rf / # not a shell".into(),
            model: Some("claude-opus-4-8".into()),
            project_id: None,
            max_turns: 12,
            tool_scope: scope,
            provider: AgentProvider::ClaudeCode,
            context: None,
            workspace_dir: None,
            conversation_id: None,
            resume_session_id: resume.map(str::to_string),
            api_key: api_key.map(str::to_string),
        }
    }

    #[test]
    fn conversation_uses_stable_cwd_dirname() {
        // Pure check of the dir-name rule used for the cwd (the actual dir is made
        // in `run`): a conversation → `conv-<id>`, else `run-<run_id>`.
        let mut s = spec(None, ToolScope::KbOnly, None);
        assert_eq!(super::conv_dirname(&s), "run-run-1");
        s.conversation_id = Some("abc-123".into());
        assert_eq!(super::conv_dirname(&s), "conv-abc-123");
    }

    #[test]
    fn granted_workspace_adds_dir_and_read_tools() {
        let cfg = CliConfig::default();
        let mut s = spec(None, ToolScope::KbOnly, None);
        s.workspace_dir = Some("/home/bart/Pictures".into());
        let argv = build_argv(&cfg, &s, Path::new("/sb"), Path::new("/sb/mcp.json"));

        // The granted folder is added as a working dir.
        let dirs: Vec<&String> = argv
            .iter()
            .enumerate()
            .filter(|(i, _)| *i > 0 && argv[i - 1] == "--add-dir")
            .map(|(_, v)| v)
            .collect();
        assert!(dirs.iter().any(|d| *d == "/home/bart/Pictures"));

        // Read-only file tools are pre-approved; host tools still denied.
        let i = argv.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(argv[i + 1], "mcp__baitler,Read,Glob,Grep");
        assert!(argv.join(" ").contains("--disallowedTools Bash,Write,Edit"));

        // The agent is TOLD about the folder, so an unqualified "list the files"
        // resolves to it instead of the empty sandbox cwd.
        let i = argv
            .iter()
            .position(|a| a == "--append-system-prompt")
            .unwrap();
        assert!(argv[i + 1].contains("/home/bart/Pictures"));
        assert!(argv[i + 1].contains("read-only"));
    }

    /// The one `--append-system-prompt` value for a spec.
    fn system_prompt(cfg: &CliConfig, s: &RunSpec) -> String {
        let argv = build_argv(cfg, s, Path::new("/sb"), Path::new("/sb/mcp.json"));
        assert_eq!(
            argv.iter()
                .filter(|a| *a == "--append-system-prompt")
                .count(),
            1
        );
        let i = argv
            .iter()
            .position(|a| a == "--append-system-prompt")
            .unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn system_prompt_states_constraints_and_context() {
        let cfg = CliConfig::default();

        // Every run gets the operating-constraints preamble; with no folder it
        // says so explicitly (the headless run can't be granted access later).
        let plain = system_prompt(&cfg, &spec(None, ToolScope::KbOnly, None));
        assert!(plain.starts_with("You are Baitler's butler agent"));
        assert!(plain.contains("NEVER be approved"));
        assert!(plain.contains("NO local-disk access"));

        // Caller context (e.g. the current app page) is appended after.
        let mut s = spec(None, ToolScope::KbOnly, None);
        s.context = Some("The user is on the Files page.".into());
        let p = system_prompt(&cfg, &s);
        assert!(p.starts_with("You are Baitler's butler agent"));
        assert!(p.ends_with("The user is on the Files page."));

        // An attached folder replaces the no-access note with the grant.
        s.workspace_dir = Some("/home/bart/Pictures".into());
        let p = system_prompt(&cfg, &s);
        assert!(p.contains("/home/bart/Pictures"));
        assert!(!p.contains("NO local-disk access"));
        assert!(p.ends_with("The user is on the Files page."));
    }

    #[test]
    fn provider_env_claude_vs_minimax() {
        // Claude Code with a key → just ANTHROPIC_API_KEY; no key → nothing
        // (inherit host auth). Model slots are NOT overridden (Anthropic has the
        // real small/haiku models).
        let c = provider_env(
            AgentProvider::ClaudeCode,
            "ignored",
            Some("claude-opus-4-8"),
            Some("sk-ant-x"),
        );
        assert_eq!(c, vec![("ANTHROPIC_API_KEY", "sk-ant-x".to_string())]);
        assert!(provider_env(AgentProvider::ClaudeCode, "ignored", None, None).is_empty());

        // MiniMax points the same CLI at its endpoint with the subscription token
        // AND maps every model slot to the MiniMax model (so background/small-model
        // calls don't 404 on a Claude model name).
        let m = provider_env(
            AgentProvider::Minimax,
            "https://api.minimax.io/anthropic",
            Some("MiniMax-M3"),
            Some("sk-cp-x"),
        );
        assert!(m.contains(&(
            "ANTHROPIC_BASE_URL",
            "https://api.minimax.io/anthropic".to_string()
        )));
        assert!(m.contains(&("ANTHROPIC_AUTH_TOKEN", "sk-cp-x".to_string())));
        assert!(m.contains(&("ANTHROPIC_DEFAULT_HAIKU_MODEL", "MiniMax-M3".to_string())));
        assert!(m.contains(&("ANTHROPIC_SMALL_FAST_MODEL", "MiniMax-M3".to_string())));
        assert!(m.contains(&("ANTHROPIC_MODEL", "MiniMax-M3".to_string())));
    }

    #[test]
    fn argv_is_locked_down_by_default() {
        let cfg = CliConfig::default(); // allow_host_tools = false
        let spec = spec(Some("sk-ant-SECRET"), ToolScope::KbOnly, None);
        let argv = build_argv(&cfg, &spec, Path::new("/sb"), Path::new("/sb/mcp.json"));

        // Never bypass permissions.
        assert!(!argv.iter().any(|a| a == "--dangerously-skip-permissions"));
        // Host tools denied.
        let joined = argv.join(" ");
        assert!(joined.contains("--disallowedTools Bash,Write,Edit"));
        // Strict MCP config + loopback wiring.
        assert!(argv.iter().any(|a| a == "--strict-mcp-config"));
        assert!(argv.iter().any(|a| a == "/sb/mcp.json"));
        // Only the Baitler MCP server is pre-approved.
        let i = argv.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(argv[i + 1], "mcp__baitler");
        // The prompt is a discrete argv element (no shell to inject into).
        assert!(argv.contains(&spec.prompt));
        // The API key is NEVER in argv.
        assert!(!argv.iter().any(|a| a.contains("SECRET")));
    }

    #[test]
    fn host_tools_optin_and_read_scope() {
        let cfg = CliConfig {
            allow_host_tools: true,
            ..CliConfig::default()
        };
        let spec = spec(None, ToolScope::KbPlusRead, Some("sess-9"));
        let argv = build_argv(&cfg, &spec, Path::new("/sb"), Path::new("/sb/mcp.json"));

        // No deny list when host tools are opted in.
        assert!(!argv.iter().any(|a| a == "--disallowedTools"));
        // Read added to the allow list.
        let i = argv.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(argv[i + 1], "mcp__baitler,Read");
        // Resume threads the prior session.
        let r = argv.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(argv[r + 1], "sess-9");
    }

    #[test]
    fn mcp_config_points_at_loopback_with_attribution() {
        let cfg = build_mcp_config("http://127.0.0.1:8080/mcp", Some("tok"), "run-42");
        let server = &cfg["mcpServers"]["baitler"];
        assert_eq!(server["type"], "http");
        assert_eq!(server["url"], "http://127.0.0.1:8080/mcp");
        assert_eq!(server["headers"]["X-Baitler-Agent"], AGENT_LABEL);
        assert_eq!(server["headers"]["X-Baitler-Run"], "run-42");
        assert_eq!(server["headers"]["Authorization"], "Bearer tok");

        // No token → no Authorization header.
        let open = build_mcp_config("http://x/mcp", None, "run-42");
        assert!(open["mcpServers"]["baitler"]["headers"]
            .get("Authorization")
            .is_none());
    }

    #[test]
    fn parse_line_maps_event_types() {
        let init =
            parse_line(r#"{"type":"system","subtype":"init","session_id":"s1","model":"m1"}"#);
        assert!(
            matches!(&init[..], [RunEvent::Init { session_id: Some(s), model: Some(m) }] if s == "s1" && m == "m1")
        );

        let asst = parse_line(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"},{"type":"tool_use","name":"mcp__baitler__ideas_create","input":{"title":"x"}}]}}"#,
        );
        assert_eq!(asst.len(), 2);
        assert!(matches!(&asst[0], RunEvent::Assistant { text } if text == "hi"));
        assert!(
            matches!(&asst[1], RunEvent::ToolUse { name, .. } if name == "mcp__baitler__ideas_create")
        );

        let tr = parse_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","is_error":false,"content":"ok"}]}}"#,
        );
        assert!(matches!(&tr[..], [RunEvent::ToolResult { ok: true, .. }]));

        let res = parse_line(
            r#"{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"s1","num_turns":3,"total_cost_usd":0.02}"#,
        );
        assert!(matches!(
            &res[..],
            [RunEvent::Result {
                is_error: false,
                num_turns: 3,
                ..
            }]
        ));

        // Unknown / malformed lines are ignored, never panic.
        assert!(parse_line(r#"{"type":"mystery"}"#).is_empty());
        assert!(parse_line("not json").is_empty());
        assert!(parse_line("").is_empty());
    }
}
