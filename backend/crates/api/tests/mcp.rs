//! End-to-end tests for the built-in MCP server (`POST /mcp`).
//!
//! Each test boots a full app instance (embedded `memory` DB) on an ephemeral
//! port and drives the MCP endpoint over real HTTP, exercising the JSON-RPC
//! handshake, tool listing, tool calls (round-tripping through the repos), the
//! `GET` 405, and bearer-token auth.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{Config, McpConfig, StorageConfig, SurrealConfig};
use baitler_api::AppState;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;

fn test_config(mcp: McpConfig) -> Config {
    Config {
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        cors_allowed_origins: vec!["http://localhost:5173".to_string()],
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
                .join("baitler-test-mcp-files")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp,
        secret_key: [7u8; 32],
    }
}

async fn spawn_app(mcp: McpConfig) -> String {
    let state: AppState = baitler_api::build_state(test_config(mcp))
        .await
        .expect("failed to build app state");
    let app = baitler_api::build_app(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server error");
    });
    format!("http://{addr}")
}

fn enabled() -> McpConfig {
    McpConfig {
        enabled: true,
        auth_token: None,
    }
}

/// POST a JSON-RPC message and return (status, parsed-body-or-null).
async fn rpc(client: &Client, base: &str, body: Value) -> (StatusCode, Value) {
    let resp = client
        .post(format!("{base}/mcp"))
        .json(&body)
        .send()
        .await
        .expect("request failed");
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let value = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).expect("response was not JSON")
    };
    (status, value)
}

fn initialize_msg() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }
    })
}

#[tokio::test]
async fn initialize_returns_server_info_and_tools_capability() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let (status, body) = rpc(&client, &base, initialize_msg()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 1);
    assert_eq!(body["result"]["serverInfo"]["name"], "baitler");
    assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    assert!(body["result"]["capabilities"]["tools"].is_object());
}

#[tokio::test]
async fn initialized_notification_is_accepted_with_no_body() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let (status, body) = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(body.is_null());
}

#[tokio::test]
async fn tools_list_advertises_known_tools() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let (status, body) = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tools = body["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for expected in [
        "ideas_create",
        "ideas_list",
        "documents_create",
        "export",
        "health",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }
    // Every tool advertises an object input schema.
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object");
    }
}

#[tokio::test]
async fn tools_call_create_then_list_idea_round_trips() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    // Create an idea via tools/call.
    let (status, body) = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {
                "name": "ideas_create",
                "arguments": { "title": "From MCP", "body": "hello", "tags": ["mcp", "test"] }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["isError"], false);
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let created: Value = serde_json::from_str(text).expect("tool returned JSON text");
    assert_eq!(created["title"], "From MCP");
    let id = created["id"].as_str().expect("idea id").to_string();

    // List ideas; the new one should be present.
    let (_s, body) = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "ideas_list", "arguments": {} }
        }),
    )
    .await;
    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    let listing: Value = serde_json::from_str(text).unwrap();
    let ideas = listing["ideas"].as_array().unwrap();
    assert!(
        ideas.iter().any(|i| i["id"] == id),
        "created idea not listed"
    );
}

#[tokio::test]
async fn tools_call_validation_error_is_is_error_result() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    // Missing required `title`.
    let (status, body) = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "ideas_create", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Tool/validation errors are results with isError:true, not JSON-RPC errors.
    assert!(body.get("error").is_none());
    assert_eq!(body["result"]["isError"], true);
}

#[tokio::test]
async fn unknown_tool_is_json_rpc_error() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let (status, body) = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": { "name": "does_not_exist", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let (_s, body) = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "id": 7, "method": "no/such/method" }),
    )
    .await;
    assert_eq!(body["error"]["code"], -32601);
}

#[tokio::test]
async fn export_tool_round_trips_markdown_to_html() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let (_s, body) = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0", "id": 8, "method": "tools/call",
            "params": {
                "name": "export",
                "arguments": { "content": "# Title\n\nbody", "source": "markdown", "target": "html" }
            }
        }),
    )
    .await;
    assert_eq!(body["result"]["isError"], false);
    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["content_type"], "text/html; charset=utf-8");
    assert!(payload["text"].as_str().unwrap().contains("<h1>"));
}

#[tokio::test]
async fn get_mcp_is_method_not_allowed() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let resp = client.get(format!("{base}/mcp")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn invalid_json_yields_parse_error() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();

    let resp = client
        .post(format!("{base}/mcp"))
        .header("content-type", "application/json")
        .body("{ not json")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], -32700);
}

#[tokio::test]
async fn bearer_token_is_enforced_when_configured() {
    let base = spawn_app(McpConfig {
        enabled: true,
        auth_token: Some("topsecret".to_string()),
    })
    .await;
    let client = Client::new();

    // No token → 401.
    let resp = client
        .post(format!("{base}/mcp"))
        .json(&initialize_msg())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Wrong token → 401.
    let resp = client
        .post(format!("{base}/mcp"))
        .bearer_auth("nope")
        .json(&initialize_msg())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Correct token → 200.
    let resp = client
        .post(format!("{base}/mcp"))
        .bearer_auth("topsecret")
        .json(&initialize_msg())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "baitler");
}

#[tokio::test]
async fn disabled_mcp_returns_not_found() {
    let base = spawn_app(McpConfig {
        enabled: false,
        auth_token: None,
    })
    .await;
    let client = Client::new();

    let (status, _body) = rpc(&client, &base, initialize_msg()).await;
    // Endpoint isn't mounted → app's JSON 404 envelope.
    assert_eq!(status, StatusCode::NOT_FOUND);
}
