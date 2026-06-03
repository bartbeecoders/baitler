//! End-to-end tests for the built-in MCP server (`POST /mcp`).
//!
//! Each test boots a full app instance (embedded `memory` DB) on an ephemeral
//! port and drives the MCP endpoint over real HTTP, exercising the JSON-RPC
//! handshake, tool listing, tool calls (round-tripping through the repos), the
//! `GET` 405, and bearer-token auth.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{CliConfig, Config, McpConfig, StorageConfig, SurrealConfig};
use baitler_api::AppState;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;

fn test_config(mcp: McpConfig) -> Config {
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
                .join("baitler-test-mcp-files")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp,
        cli: CliConfig::default(),
        public_page_origin: None,
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

/// Call a tool as a named agent (sends `X-Baitler-Agent`), assert success, and
/// return the tool's structured result (parsed from the text content).
async fn tool_call(client: &Client, base: &str, agent: &str, name: &str, args: Value) -> Value {
    let resp = client
        .post(format!("{base}/mcp"))
        .header("X-Baitler-Agent", agent)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": args }
        }))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("json body");
    assert_eq!(
        body["result"]["isError"], false,
        "tool `{name}` returned an error: {body}"
    );
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    serde_json::from_str(text).expect("tool result was not JSON")
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

/// The full Phase 11 agentic loop, driven over JSON-RPC exactly as an external
/// agent (Claude Code, Hermes, …) would — organise → connect → retrieve →
/// publish → export → grounded chat — with provenance asserted at the end.
/// Fully offline: the Mock LLM provider is the chat path; no egress, no keys.
#[tokio::test]
async fn agentic_loop_end_to_end() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();
    let agent = "claude-code-test";

    // tools/list advertises the Phase 11 surface.
    let (_s, listed) = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    )
    .await;
    let names: Vec<String> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for expected in [
        "projects_create",
        "projects_add_item",
        "knowledge_link",
        "knowledge_search",
        "knowledge_backlinks",
        "activity_list",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "missing tool {expected}"
        );
    }

    // ── ORGANISE ──
    let project = tool_call(
        &client,
        &base,
        agent,
        "projects_create",
        json!({ "name": "Ownership Project", "summary": "rust memory model" }),
    )
    .await;
    let pid = project["id"].as_str().unwrap().to_string();
    assert_eq!(project["slug"], "ownership-project");

    let folder = tool_call(
        &client,
        &base,
        agent,
        "folders_create",
        json!({ "name": "assets" }),
    )
    .await;
    let fid = folder["id"].as_str().unwrap().to_string();
    let file = tool_call(
        &client,
        &base,
        agent,
        "files_write",
        json!({ "name": "notes.txt", "content_text": "raw notes", "folder": fid }),
    )
    .await;
    tool_call(
        &client,
        &base,
        agent,
        "projects_add_item",
        json!({ "project_id": pid, "item_type": "file", "item_id": file["id"] }),
    )
    .await;

    // Agent-authored idea + document default to review=draft and land in the project.
    let idea = tool_call(&client, &base, agent, "ideas_create",
        json!({ "title": "Borrow checker", "body": "# ownership\nthe borrow checker", "project_id": pid })).await;
    assert_eq!(idea["review"], "draft", "agent writes default to draft");
    assert_eq!(idea["project_id"], pid);
    let iid = idea["id"].as_str().unwrap().to_string();

    let doc = tool_call(&client, &base, agent, "documents_create",
        json!({ "title": "Ownership spec", "body": "<p>the ownership model and lifetimes</p>", "project_id": pid })).await;
    assert_eq!(doc["review"], "draft");
    let did = doc["id"].as_str().unwrap().to_string();

    // ── CONNECT ──
    tool_call(&client, &base, agent, "knowledge_link",
        json!({ "src_type": "idea", "src_id": iid, "dst_type": "document", "dst_id": did, "relation": "implements" })).await;

    let detail = tool_call(&client, &base, agent, "projects_get", json!({ "id": pid })).await;
    assert_eq!(detail["counts"]["ideas"], 1);
    assert_eq!(detail["counts"]["documents"], 1);
    assert_eq!(detail["counts"]["files"], 1);
    assert_eq!(
        detail["counts"]["drafts"], 2,
        "idea + document are pending approval"
    );

    // Backlinks resolve from the idea to the document.
    let backlinks = tool_call(
        &client,
        &base,
        agent,
        "knowledge_backlinks",
        json!({ "item_type": "idea", "item_id": iid }),
    )
    .await;
    assert_eq!(backlinks["links"][0]["id"], did);
    assert_eq!(backlinks["links"][0]["relation"], "implements");

    // ── RETRIEVE ──
    let search = tool_call(
        &client,
        &base,
        agent,
        "knowledge_search",
        json!({ "q": "ownership" }),
    )
    .await;
    assert!(
        !search["documents"].as_array().unwrap().is_empty(),
        "doc matches 'ownership'"
    );
    assert!(
        !search["ideas"].as_array().unwrap().is_empty(),
        "idea matches 'ownership'"
    );

    // ── APPROVE / PUBLISH ──
    let published = tool_call(
        &client,
        &base,
        agent,
        "ideas_update",
        json!({ "id": iid, "review": "published" }),
    )
    .await;
    assert_eq!(published["review"], "published");

    // ── EXPORT ── (markdown works offline; pdf/docx need Chrome/Pandoc)
    let export = tool_call(
        &client,
        &base,
        agent,
        "documents_export",
        json!({ "id": did, "format": "markdown" }),
    )
    .await;
    assert_eq!(export["content_type"], "text/markdown; charset=utf-8");
    assert!(export["text"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("ownership"));

    // ── GROUNDED CHAT ── (Mock provider, offline)
    let chat = tool_call(
        &client,
        &base,
        agent,
        "ai_chat",
        json!({ "provider": "mock", "model": "mock-1",
                "messages": [{ "role": "user", "content": "summarize the project" }],
                "context": "the ownership model" }),
    )
    .await;
    assert!(chat["text"].as_str().unwrap().contains("mock"));

    // ── PROVENANCE ── every mutation is attributed to the agent.
    let activity = tool_call(&client, &base, agent, "activity_list", json!({})).await;
    let rows = activity["activity"].as_array().unwrap();
    let has = |action: &str| {
        rows.iter()
            .any(|r| r["action"] == action && r["agent"] == agent)
    };
    assert!(has("project.create"));
    assert!(has("idea.create"));
    assert!(has("document.create"));
    assert!(has("knowledge.link"));
    assert!(has("idea.update"), "the publish/approve was recorded");

    // Filtering by agent returns only this agent's actions; reads logged nothing.
    let by_agent = tool_call(
        &client,
        &base,
        agent,
        "activity_list",
        json!({ "agent": agent }),
    )
    .await;
    assert!(by_agent["activity"].as_array().unwrap().len() >= 6);
    assert!(
        rows.iter()
            .all(|r| r["action"].as_str().unwrap() != "knowledge.search"),
        "read-only tools record no activity"
    );
}

/// Phase 12 Milestone C: an agent authors a web page under a project over MCP,
/// it is findable by knowledge_search, publishing returns a shareable URL that
/// `GET /p/{slug}` serves, the publish is attributed in the activity log, and
/// unpublishing 404s the URL. Fully offline; no egress.
#[tokio::test]
async fn pages_mcp_authoring_publish_and_serve() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();
    let agent = "page-author";

    // tools/list advertises the full pages surface.
    let (_s, listed) = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    )
    .await;
    let names: Vec<String> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for expected in [
        "pages_list",
        "pages_get",
        "pages_create",
        "pages_update",
        "pages_delete",
        "pages_publish",
        "pages_unpublish",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "missing tool {expected}"
        );
    }

    let project = tool_call(
        &client,
        &base,
        agent,
        "projects_create",
        json!({ "name": "Site" }),
    )
    .await;
    let pid = project["id"].as_str().unwrap().to_string();

    // Author from Markdown; visibility defaults to draft (never self-published).
    let page = tool_call(
        &client,
        &base,
        agent,
        "pages_create",
        json!({ "title": "Launch Notes", "body": "# Launch\n\nwe shipped", "source_format": "markdown", "project_id": pid }),
    )
    .await;
    assert_eq!(page["visibility"], "draft", "agent page defaults to draft");
    assert_eq!(page["public_url"], "", "a draft has no public url");
    assert!(
        page["body"].as_str().unwrap().contains("<h1>"),
        "markdown rendered to HTML"
    );
    assert_eq!(page["project_id"], pid);
    let page_id = page["id"].as_str().unwrap().to_string();

    // It's a project member and findable by knowledge_search (the page FT section).
    let detail = tool_call(&client, &base, agent, "projects_get", json!({ "id": pid })).await;
    assert_eq!(detail["counts"]["pages"], 1);
    let search = tool_call(
        &client,
        &base,
        agent,
        "knowledge_search",
        json!({ "q": "launch" }),
    )
    .await;
    assert!(
        !search["pages"].as_array().unwrap().is_empty(),
        "page matches 'launch' in knowledge_search"
    );

    // Publish → a non-empty shareable URL (id + title for provenance).
    let published = tool_call(
        &client,
        &base,
        agent,
        "pages_publish",
        json!({ "id": page_id }),
    )
    .await;
    assert_eq!(published["published"], true);
    assert_eq!(published["visibility"], "public");
    assert_eq!(published["id"], page_id);
    assert_eq!(published["title"], "Launch Notes");
    let url = published["url"].as_str().unwrap().to_string();
    assert!(!url.is_empty(), "publish returns a shareable url");
    assert!(url.starts_with("/p/"));

    // The URL serves the published page.
    let served = client.get(format!("{base}{url}")).send().await.unwrap();
    assert_eq!(served.status(), StatusCode::OK);
    assert!(served.text().await.unwrap().contains("we shipped"));

    // Provenance: page.create + page.publish attributed to the agent, with id+title.
    let activity = tool_call(
        &client,
        &base,
        agent,
        "activity_list",
        json!({ "agent": agent }),
    )
    .await;
    let rows = activity["activity"].as_array().unwrap();
    let publish_row = rows
        .iter()
        .find(|r| r["action"] == "page.publish")
        .expect("page.publish logged");
    assert_eq!(publish_row["agent"], agent);
    assert_eq!(
        publish_row["target_id"], page_id,
        "publish row carries the page id"
    );
    assert_eq!(publish_row["target_title"], "Launch Notes");
    assert!(rows.iter().any(|r| r["action"] == "page.create"));

    // Unpublish → the URL immediately 404s.
    tool_call(
        &client,
        &base,
        agent,
        "pages_unpublish",
        json!({ "id": page_id }),
    )
    .await;
    let after = client.get(format!("{base}{url}")).send().await.unwrap();
    assert_eq!(after.status(), StatusCode::NOT_FOUND);
}

/// Publishing: render a document and a whole project to stored files, and flip
/// the document's review state. Uses markdown (no Chrome/Pandoc needed → CI-safe).
#[tokio::test]
async fn publishing_persists_artifacts_and_marks_published() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();
    let agent = "publisher";

    let project = tool_call(
        &client,
        &base,
        agent,
        "projects_create",
        json!({ "name": "Handbook", "summary": "the team handbook" }),
    )
    .await;
    let pid = project["id"].as_str().unwrap().to_string();

    let doc = tool_call(
        &client,
        &base,
        agent,
        "documents_create",
        json!({ "title": "Chapter 1", "body": "<h1>Intro</h1><p>hello</p>", "project_id": pid }),
    )
    .await;
    assert_eq!(doc["review"], "draft");
    let did = doc["id"].as_str().unwrap().to_string();

    // Publish the document as markdown → a stored file, review flips to published.
    let published = tool_call(
        &client,
        &base,
        agent,
        "documents_publish",
        json!({ "id": did, "format": "markdown" }),
    )
    .await;
    assert_eq!(published["published"], true);
    let file = &published["file"];
    assert!(file["name"].as_str().unwrap().ends_with(".md"));
    assert_eq!(file["mime"], "text/markdown; charset=utf-8");
    let fid = file["id"].as_str().unwrap().to_string();

    // The artifact is a real, listable, readable file.
    let listed = tool_call(&client, &base, agent, "files_list", json!({})).await;
    assert!(listed["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == fid));

    // The document is now published.
    let got = tool_call(&client, &base, agent, "documents_get", json!({ "id": did })).await;
    assert_eq!(got["review"], "published");

    // Export the whole project to one markdown artifact (title + contents + the doc).
    let collection = tool_call(
        &client,
        &base,
        agent,
        "collection_export",
        json!({ "project_id": pid, "format": "markdown" }),
    )
    .await;
    assert_eq!(collection["exported"], true);
    assert_eq!(collection["documents"], 1);
    assert!(collection["file"]["name"]
        .as_str()
        .unwrap()
        .ends_with(".md"));

    // Both actions are attributed to the agent.
    let activity = tool_call(
        &client,
        &base,
        agent,
        "activity_list",
        json!({ "agent": agent }),
    )
    .await;
    let actions: Vec<&str> = activity["activity"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["action"].as_str())
        .collect();
    assert!(actions.contains(&"document.publish"));
    assert!(actions.contains(&"project.export"));
}

#[tokio::test]
async fn files_import_reads_local_files_server_side() {
    // A temp dir (the allow-listed root) with two files.
    let root = std::env::temp_dir().join("baitler-mcp-import-test");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.png"), b"\x89PNG not-a-real-image").unwrap();
    std::fs::write(root.join("b.txt"), b"hello world").unwrap();

    let mut cfg = test_config(enabled());
    cfg.cli.workspace_roots = vec![root.to_string_lossy().into_owned()];
    let state = baitler_api::build_state(cfg).await.expect("state");
    let app = baitler_api::build_app(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    let client = Client::new();

    // Import the whole directory — the server reads the bytes itself.
    let res = tool_call(
        &client,
        &base,
        "claude-code",
        "files_import",
        json!({ "path": root.to_string_lossy() }),
    )
    .await;
    assert_eq!(res["count"], 2, "imported both files: {res}");
    let names: Vec<&str> = res["imported"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["name"].as_str())
        .collect();
    assert!(names.contains(&"a.png"));
    assert!(names.contains(&"b.txt"));

    // They now live in Baitler Files (at the root).
    let list = tool_call(&client, &base, "claude-code", "files_list", json!({})).await;
    assert_eq!(list["files"].as_array().unwrap().len(), 2);

    // A path OUTSIDE the allow-listed root is refused.
    let resp = client
        .post(format!("{base}/mcp"))
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "files_import", "arguments": { "path": std::env::temp_dir().to_string_lossy() } }
        }))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["result"]["isError"], true,
        "out-of-root import must fail"
    );
}

/// Phase 14, Milestone B/C: the agentic mindmap + diagram surface. An agent
/// lists the new tools, seeds a mindmap from a project (ideas → nodes), authors
/// a draft diagram, cross-links the two, and confirms a `page.publish`-style
/// activity attribution + a `kn_link` scrub on delete. Fully offline.
#[tokio::test]
async fn mindmaps_and_diagrams_mcp_surface() {
    let base = spawn_app(enabled()).await;
    let client = Client::new();
    let agent = "modeler";

    // tools/list advertises the new tools.
    let (_s, listed) = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    )
    .await;
    let names: Vec<String> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for expected in [
        "mindmaps_list",
        "mindmaps_get",
        "mindmaps_create",
        "mindmaps_update",
        "mindmaps_delete",
        "mindmaps_from_project",
        "diagrams_list",
        "diagrams_get",
        "diagrams_create",
        "diagrams_update",
        "diagrams_delete",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "missing tool {expected}"
        );
    }

    // A project with two ideas as members.
    let project = tool_call(
        &client,
        &base,
        agent,
        "projects_create",
        json!({ "name": "Atlas" }),
    )
    .await;
    let pid = project["id"].as_str().unwrap().to_string();
    let mut idea_ids = Vec::new();
    for t in ["North", "South"] {
        let idea = tool_call(
            &client,
            &base,
            agent,
            "ideas_create",
            json!({ "title": t, "review": "published", "project_id": pid }),
        )
        .await;
        idea_ids.push(idea["id"].as_str().unwrap().to_string());
    }

    // Seed a mindmap from the project: root + 2 ideas.
    let mm = tool_call(
        &client,
        &base,
        agent,
        "mindmaps_from_project",
        json!({ "project_id": pid }),
    )
    .await;
    let mm_id = mm["id"].as_str().unwrap().to_string();
    assert_eq!(mm["review"], "draft", "agent mindmap defaults to draft");
    assert_eq!(mm["graph"]["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(mm["project_id"], pid);

    // Author a draft diagram with extractable labels.
    let diagram = tool_call(
        &client,
        &base,
        agent,
        "diagrams_create",
        json!({
            "title": "Topology",
            "xml": "<mxGraphModel><root><mxCell id=\"2\" value=\"Gateway\" vertex=\"1\"/></root></mxGraphModel>",
            "project_id": pid
        }),
    )
    .await;
    let diagram_id = diagram["id"].as_str().unwrap().to_string();
    assert_eq!(diagram["review"], "draft");

    // Cross-link the mindmap and the diagram (proves both are valid item types).
    let linked = tool_call(
        &client,
        &base,
        agent,
        "knowledge_link",
        json!({ "src_type": "mindmap", "src_id": mm_id, "dst_type": "diagram", "dst_id": diagram_id, "relation": "depicts" }),
    )
    .await;
    assert_eq!(linked["linked"], true);

    // knowledge_search surfaces both the mindmap (by node label) and the diagram (by XML label).
    let search = tool_call(
        &client,
        &base,
        agent,
        "knowledge_search",
        json!({ "q": "North" }),
    )
    .await;
    assert!(search["mindmaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["id"] == mm_id));
    let search = tool_call(
        &client,
        &base,
        agent,
        "knowledge_search",
        json!({ "q": "Gateway" }),
    )
    .await;
    assert!(search["diagrams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["id"] == diagram_id));

    // Activity recorded the creates, attributed to the agent.
    let activity = tool_call(
        &client,
        &base,
        agent,
        "activity_list",
        json!({ "agent": agent }),
    )
    .await;
    let actions: Vec<&str> = activity["activity"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["action"].as_str().unwrap())
        .collect();
    assert!(
        actions.contains(&"mindmap.create"),
        "missing mindmap.create: {actions:?}"
    );
    assert!(
        actions.contains(&"diagram.create"),
        "missing diagram.create: {actions:?}"
    );

    // Deleting the mindmap scrubs the cross-type link (the diagram's backlinks drop it).
    let del = tool_call(
        &client,
        &base,
        agent,
        "mindmaps_delete",
        json!({ "id": mm_id }),
    )
    .await;
    assert_eq!(del["deleted"], true);
    let backlinks = tool_call(
        &client,
        &base,
        agent,
        "knowledge_backlinks",
        json!({ "item_type": "diagram", "item_id": diagram_id }),
    )
    .await;
    assert!(
        backlinks["links"].as_array().unwrap().is_empty(),
        "link should be scrubbed when the mindmap is deleted: {backlinks}"
    );
}
