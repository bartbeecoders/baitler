//! Integration tests for the Claude Code CLI runner (Phase 13, Milestone A).
//!
//! Everything runs against the in-process `MockRunner` (`CLAUDE_BIN == "mock"`) on
//! an embedded `memory` DB — no `claude` binary, no network, no Anthropic key.
//! The real `ClaudeCliRunner` is unexercised here by design (it needs egress);
//! its security-critical pure functions are unit-tested in `src/cli/claude.rs`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{CliConfig, Config, McpConfig, StorageConfig, SurrealConfig};
use baitler_api::AppState;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;

fn test_config(cli_enabled: bool) -> Config {
    Config {
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        cors_allowed_origins: vec!["http://localhost:8100".to_string()],
        db_timeout: Duration::from_secs(5),
        surreal: SurrealConfig {
            url: "memory".to_string(),
            username: None,
            password: None,
            namespace: "test".to_string(),
            database: "test".to_string(),
        },
        storage: StorageConfig {
            backend: "local".to_string(),
            local_path: std::env::temp_dir()
                .join("baitler-test-cli")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp: McpConfig {
            enabled: true,
            auth_token: None,
        },
        // `bin = "mock"` selects the in-process runner; `enabled` toggles the 503.
        cli: CliConfig {
            enabled: cli_enabled,
            ..CliConfig::default()
        },
        public_page_origin: None,
        secret_key: [7u8; 32],
    }
}

async fn spawn(cli_enabled: bool) -> (String, AppState) {
    let state = baitler_api::build_state(test_config(cli_enabled))
        .await
        .expect("state");
    let app = baitler_api::build_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (format!("http://{addr}"), state)
}

async fn post(c: &Client, url: String, body: Value) -> reqwest::Response {
    c.post(url).json(&body).send().await.expect("post")
}

/// Parse an SSE body into the ordered list of event JSON objects.
fn parse_sse(body: &str) -> Vec<Value> {
    body.lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

fn types(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| e["type"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn run_streams_events_and_persists_outcome() {
    let (base, _state) = spawn(true).await;
    let c = Client::new();

    let resp = post(
        &c,
        format!("{base}/cli/runs"),
        json!({ "prompt": "Document my project", "model": "mock-x" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    let events = parse_sse(&body);
    let seq = types(&events);

    // The mock scripts a full run: init → assistant → tool_use → tool_result →
    // result → done.
    assert!(seq.contains(&"init".to_string()), "events: {seq:?}");
    assert!(seq.contains(&"assistant".to_string()));
    assert!(seq.contains(&"tool_use".to_string()));
    assert!(seq.contains(&"tool_result".to_string()));
    assert!(seq.contains(&"result".to_string()));
    assert_eq!(seq.last().map(String::as_str), Some("done"));

    let done = events.last().unwrap();
    assert_eq!(done["status"], "succeeded");

    // The run is listed and the row carries the terminal outcome + session id.
    let list: Value = c
        .get(format!("{base}/cli/runs"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let runs = list["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    let id = runs[0]["id"].as_str().unwrap();
    assert_eq!(runs[0]["status"], "succeeded");

    let detail: Value = c
        .get(format!("{base}/cli/runs/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["status"], "succeeded");
    assert_eq!(detail["num_turns"], 2);
    assert_eq!(detail["session_id"], format!("mock-{id}"));
    assert!(detail["result_text"].as_str().unwrap().contains("Mock run"));
    assert!(detail["finished_at"].is_string());
}

#[tokio::test]
async fn status_reports_readiness() {
    // Mock runner enabled → ready, no key required.
    let (base, _state) = spawn(true).await;
    let c = Client::new();
    let s: Value = c
        .get(format!("{base}/cli/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(s["enabled"], true);
    assert_eq!(s["kind"], "mock");
    assert_eq!(s["ready"], true);

    // Disabled → not ready, with a hint.
    let (base, _state) = spawn(false).await;
    let s: Value = c
        .get(format!("{base}/cli/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(s["enabled"], false);
    assert_eq!(s["ready"], false);
    assert!(s["message"]
        .as_str()
        .unwrap()
        .contains("CLAUDE_CLI_ENABLED"));

    // Both agent providers are advertised (mock makes minimax available too).
    let (base, _state) = spawn(true).await;
    let s: Value = c
        .get(format!("{base}/cli/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = s["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"claude_code"));
    assert!(ids.contains(&"minimax"));
}

#[tokio::test]
async fn minimax_runs_via_mock_and_records_model() {
    // With the mock runner, the minimax provider works without a key and the
    // requested model is persisted (the route resolves it from the body).
    let (base, _state) = spawn(true).await;
    let c = Client::new();
    let resp = post(
        &c,
        format!("{base}/cli/runs"),
        json!({ "prompt": "do it", "provider": "minimax" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let events = parse_sse(&resp.text().await.unwrap());
    assert_eq!(types(&events).last().map(String::as_str), Some("done"));

    let list: Value = c
        .get(format!("{base}/cli/runs"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // The minimax default model (MiniMax-M3) was recorded on the row.
    assert_eq!(list["runs"][0]["model"], "MiniMax-M3");
}

#[tokio::test]
async fn workspace_grant_is_validated_against_roots() {
    // Configure an allow-listed root (a temp dir) and a folder inside it.
    let root = std::env::temp_dir().join("baitler-cli-ws-int");
    let inside = root.join("pics");
    std::fs::create_dir_all(&inside).unwrap();

    let mut cfg = test_config(true);
    cfg.cli.workspace_roots = vec![root.to_string_lossy().into_owned()];
    let state = baitler_api::build_state(cfg).await.expect("state");
    let app = baitler_api::build_app(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    let c = Client::new();

    // A folder inside the root is accepted (mock runner streams to done).
    let ok = post(
        &c,
        format!("{base}/cli/runs"),
        json!({ "prompt": "import pics", "workspace_dir": inside.to_string_lossy() }),
    )
    .await;
    assert_eq!(ok.status(), StatusCode::OK);

    // A folder OUTSIDE the root is rejected.
    let bad = post(
        &c,
        format!("{base}/cli/runs"),
        json!({ "prompt": "x", "workspace_dir": std::env::temp_dir().to_string_lossy() }),
    )
    .await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

    // status advertises the configured root.
    let s: Value = c
        .get(format!("{base}/cli/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!s["workspace_roots"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn conversation_persists_dir_and_records_id() {
    // A conversation reuses a stable working dir across turns (so --resume works),
    // and the conversation_id is recorded on the run row.
    let workdir = std::env::temp_dir().join("baitler-cli-conv-test");
    let _ = std::fs::remove_dir_all(&workdir);

    let mut cfg = test_config(true);
    // Use the REAL runner path? No — keep the mock (no binary). But the mock
    // doesn't create a sandbox dir; the dir/cwd logic lives in the real runner.
    // So here we just assert the row carries conversation_id end-to-end.
    cfg.cli.workdir = workdir.to_string_lossy().into_owned();
    let state = baitler_api::build_state(cfg).await.expect("state");
    let app = baitler_api::build_app(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    let c = Client::new();

    let r = post(
        &c,
        format!("{base}/cli/runs"),
        json!({ "prompt": "hello", "conversation_id": "chat-123" }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    let _ = r.text().await;

    let list: Value = c
        .get(format!("{base}/cli/runs"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["runs"][0]["conversation_id"], "chat-123");
}

#[tokio::test]
async fn invalid_provider_is_rejected() {
    let (base, _state) = spawn(true).await;
    let c = Client::new();
    let resp = post(
        &c,
        format!("{base}/cli/runs"),
        json!({ "prompt": "x", "provider": "gpt" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn disabled_runner_returns_503() {
    let (base, _state) = spawn(false).await;
    let c = Client::new();
    let resp = post(&c, format!("{base}/cli/runs"), json!({ "prompt": "hi" })).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn empty_prompt_is_rejected() {
    let (base, _state) = spawn(true).await;
    let c = Client::new();
    let resp = post(&c, format!("{base}/cli/runs"), json!({ "prompt": "   " })).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn second_concurrent_run_is_409() {
    let (base, state) = spawn(true).await;
    let c = Client::new();

    // Occupy the dev owner's single active slot directly, deterministically.
    let _slot = state
        .cli_runs
        .begin("dev", "held-run")
        .expect("slot is free");

    let resp = post(&c, format!("{base}/cli/runs"), json!({ "prompt": "hi" })).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    state.cli_runs.finish("dev", "held-run");
    // Once the slot is free, a run starts normally.
    let ok = post(&c, format!("{base}/cli/runs"), json!({ "prompt": "hi" })).await;
    assert_eq!(ok.status(), StatusCode::OK);
}

#[tokio::test]
async fn cancel_endpoint_semantics() {
    let (base, state) = spawn(true).await;
    let c = Client::new();

    // Unknown run → 404.
    let r = post(&c, format!("{base}/cli/runs/nope/cancel"), json!({})).await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // A row that exists and is registered active → 204 + cancelled flag set.
    let run = baitler_api::cli::repo::create_run(
        &state.db,
        "dev",
        "run-x",
        "do a thing",
        None,
        "kb_only",
        None,
        None,
    )
    .await
    .expect("create run");
    let _slot = state.cli_runs.begin("dev", &run.uuid).expect("slot");

    let r = post(
        &c,
        format!("{base}/cli/runs/{}/cancel", run.uuid),
        json!({}),
    )
    .await;
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    assert!(state.cli_runs.was_cancelled(&run.uuid));
    state.cli_runs.finish("dev", &run.uuid);

    // A row that exists but isn't active → 409.
    let r = post(
        &c,
        format!("{base}/cli/runs/{}/cancel", run.uuid),
        json!({}),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn runs_are_owner_scoped() {
    // Repo-level isolation with two synthetic owners (mirrors documents/files).
    let (_base, state) = spawn(true).await;
    baitler_api::cli::repo::create_run(
        &state.db, "owner-a", "ra", "a", None, "kb_only", None, None,
    )
    .await
    .unwrap();
    baitler_api::cli::repo::create_run(
        &state.db, "owner-b", "rb", "b", None, "kb_only", None, None,
    )
    .await
    .unwrap();

    let a = baitler_api::cli::repo::list_runs(&state.db, "owner-a", None, None, 50, 0)
        .await
        .unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].uuid, "ra");

    // owner-a cannot fetch owner-b's run.
    assert!(baitler_api::cli::repo::get_run(&state.db, "owner-a", "rb")
        .await
        .unwrap()
        .is_none());
}
