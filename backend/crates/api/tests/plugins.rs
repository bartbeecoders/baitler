//! End-to-end tests for the plugin system lifecycle (Phase 16.B).
//!
//! Everything here runs WITHOUT the `plugins` Cargo feature: authoring,
//! validation diagnostics, the forced-draft invariant, the portal-only REST
//! lifecycle, dynamic tools/list advertisement, the run-actor uninstall
//! refusal, knowledge integration, and uninstall scrubbing are all
//! storage/protocol concerns. Actual WASM execution lives in
//! `plugins_runtime.rs` (feature-gated).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{
    CliConfig, Config, McpConfig, PluginsConfig, StorageConfig, SurrealConfig,
};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;

fn test_config(plugins_enabled: bool) -> Config {
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
                .join("baitler-test-plugin-files")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp: McpConfig {
            enabled: true,
            auth_token: None,
        },
        cli: CliConfig::default(),
        plugins: PluginsConfig {
            enabled: plugins_enabled,
        },
        workspace_roots: Vec::new(),
        public_page_origin: None,
        secret_key: [7u8; 32],
    }
}

async fn spawn_app(plugins_enabled: bool) -> String {
    let state = baitler_api::build_state(test_config(plugins_enabled))
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

/// `tools/call` with optional `X-Baitler-Run`; returns the raw JSON-RPC body.
async fn call_raw(
    client: &Client,
    base: &str,
    run_id: Option<&str>,
    name: &str,
    args: Value,
) -> Value {
    let mut req = client
        .post(format!("{base}/mcp"))
        .header("X-Baitler-Agent", "test-agent")
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": args }
        }));
    if let Some(run) = run_id {
        req = req.header("X-Baitler-Run", run);
    }
    let resp = req.send().await.expect("request failed");
    resp.json().await.expect("json body")
}

/// Call a tool, assert success, parse the structured result.
async fn call_ok(client: &Client, base: &str, name: &str, args: Value) -> Value {
    let body = call_raw(client, base, None, name, args).await;
    assert_eq!(
        body["result"]["isError"], false,
        "tool `{name}` errored: {body}"
    );
    serde_json::from_str(body["result"]["content"][0]["text"].as_str().unwrap())
        .expect("tool result was not JSON")
}

/// Call a tool, assert it failed, return the error text.
async fn call_err(
    client: &Client,
    base: &str,
    run_id: Option<&str>,
    name: &str,
    args: Value,
) -> String {
    let body = call_raw(client, base, run_id, name, args).await;
    assert_eq!(
        body["result"]["isError"], true,
        "tool `{name}` unexpectedly succeeded: {body}"
    );
    body["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A minimal byte-valid (empty) WASM module: enough for storage-layer tests.
fn stub_wasm_b64() -> String {
    // \0asm + version 1
    base64(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00])
}

fn base64(bytes: &[u8]) -> String {
    // Tiny local encoder (tests only; the app has its own in mcp::b64).
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn tool_manifest(name: &str) -> Value {
    json!({
        "name": name,
        "version": "0.1.0",
        "description": "test plugin",
        "tools": [{
            "name": "echo", "export": "echo", "description": "echoes",
            "input_schema": { "type": "object", "properties": {}, "required": [] }
        }],
        "capabilities": { "host_fns": ["log"] }
    })
}

#[tokio::test]
async fn disabled_plugin_system_gates_mcp_and_rest() {
    let base = spawn_app(false).await;
    let client = Client::new();

    let err = call_err(&client, &base, None, "plugins_list", json!({})).await;
    assert!(err.contains("PLUGINS_ENABLED"), "unhelpful error: {err}");

    let resp = client.get(format!("{base}/plugins")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn scaffold_validate_diagnostics_loop() {
    let base = spawn_app(true).await;
    let client = Client::new();

    let scaffold = call_ok(
        &client,
        &base,
        "plugins_scaffold",
        json!({ "name": "CSV Stats", "kind": "mcp_tool" }),
    )
    .await;
    assert!(scaffold["source"]
        .as_str()
        .unwrap()
        .contains("Host.inputString"));
    let manifest = scaffold["manifest"].clone();

    // Scaffolded tool manifest WITHOUT wasm → structured error, not a tool error.
    let invalid = call_ok(
        &client,
        &base,
        "plugins_validate",
        json!({ "manifest": manifest }),
    )
    .await;
    assert_eq!(invalid["valid"], false);
    assert!(invalid["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["msg"].as_str().unwrap().contains("no WASM module")));

    // With a module it validates clean.
    let valid = call_ok(
        &client,
        &base,
        "plugins_validate",
        json!({ "manifest": manifest, "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    assert_eq!(valid["valid"], true, "issues: {valid}");
    assert_eq!(valid["slug"], "csv-stats");

    // Unknown manifest fields (e.g. a smuggled fs grant) are rejected at parse.
    let smuggled = call_ok(
        &client,
        &base,
        "plugins_validate",
        json!({ "manifest": { "name": "x", "capabilities": { "allowed_paths": ["/"] } } }),
    )
    .await;
    assert_eq!(smuggled["valid"], false);

    // Egress/secrets/unknown host fns are all v1-denied.
    let denied = call_ok(
        &client,
        &base,
        "plugins_validate",
        json!({ "manifest": {
            "name": "x",
            "tools": [{ "name": "t", "export": "t" }],
            "capabilities": { "host_fns": ["workspace_read"], "egress": ["evil.example"] }
        }, "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    let errors = denied["errors"].as_array().unwrap();
    assert!(errors
        .iter()
        .any(|e| e["path"] == "capabilities.host_fns[0]"));
    assert!(errors.iter().any(|e| e["path"] == "capabilities.egress"));
}

#[tokio::test]
async fn create_is_forced_draft_and_portal_lifecycle_runs() {
    let base = spawn_app(true).await;
    let client = Client::new();

    // Even an explicit review=published / status=enabled is ignored: drafts.
    let created = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({
            "manifest": tool_manifest("Lifecycle Plugin"),
            "wasm_b64": stub_wasm_b64(),
            "review": "published",
            "status": "enabled"
        }),
    )
    .await;
    assert_eq!(created["status"], "draft");
    assert_eq!(created["review"], "draft");
    assert_eq!(created["author_agent"], "test-agent");
    let id = created["id"].as_str().unwrap().to_string();
    let slug = created["slug"].as_str().unwrap().to_string();

    // It surfaces in the review queue beside ideas/documents.
    let queue = call_ok(&client, &base, "review_list", json!({})).await;
    assert!(queue["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == id.as_str()));

    // …and in knowledge_search, like any first-class content type.
    let hits = call_ok(
        &client,
        &base,
        "knowledge_search",
        json!({ "q": "Lifecycle" }),
    )
    .await;
    assert!(!hits["plugins"].as_array().unwrap().is_empty());

    // tools/list does NOT advertise a draft's tools.
    let listed: Value = client
        .post(format!("{base}/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names.iter().filter(|n| n.starts_with("plugin__")).count(),
        0
    );
    assert!(names.contains(&"plugins_create")); // 89 statics incl. the meta-tools

    // A draft cannot be invoked — and the error explains the human gate.
    let err = call_err(
        &client,
        &base,
        None,
        "plugins_invoke",
        json!({ "slug": slug, "tool": "echo", "input": {} }),
    )
    .await;
    assert!(err.contains("approve"), "unhelpful: {err}");

    // Lifecycle is REST-only: draft → approved → enabled.
    let approved: Value = client
        .post(format!("{base}/plugins/{id}/approve"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(approved["status"], "approved");
    assert_eq!(approved["review"], "published");

    // Illegal edge: draft→approved→approve again.
    let twice = client
        .post(format!("{base}/plugins/{id}/approve"))
        .send()
        .await
        .unwrap();
    assert_eq!(twice.status(), StatusCode::CONFLICT);

    let enabled: Value = client
        .post(format!("{base}/plugins/{id}/enable"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(enabled["status"], "enabled");

    // NOW tools/list advertises the namespaced plugin tool, with provenance.
    let listed: Value = client
        .post(format!("{base}/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tools = listed["result"]["tools"].as_array().unwrap();
    let plugin_tool = tools
        .iter()
        .find(|t| t["name"] == format!("plugin__{slug}__echo"))
        .expect("enabled plugin tool advertised");
    assert!(plugin_tool["description"]
        .as_str()
        .unwrap()
        .starts_with(&format!("[plugin {slug} v0.1.0]")));

    // Disable unloads + de-advertises.
    let disabled: Value = client
        .post(format!("{base}/plugins/{id}/disable"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(disabled["status"], "disabled");
    let listed: Value = client
        .post(format!("{base}/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"].as_str().unwrap().starts_with("plugin__")));
}

#[tokio::test]
async fn rejected_plugins_leave_the_review_queue() {
    let base = spawn_app(true).await;
    let client = Client::new();

    let created = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({ "manifest": tool_manifest("Rejected One"), "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let queue = call_ok(&client, &base, "review_list", json!({})).await;
    assert_eq!(queue["plugins"].as_array().unwrap().len(), 1);

    // Reject parks it at status=rejected — it must NOT haunt the queue (it
    // would be un-clearable: rejected→approved is not a legal transition).
    let rejected: Value = client
        .post(format!("{base}/plugins/{id}/reject"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rejected["status"], "rejected");

    let queue = call_ok(&client, &base, "review_list", json!({})).await;
    assert_eq!(
        queue["plugins"].as_array().unwrap().len(),
        0,
        "rejected plugin still in the review queue: {queue}"
    );
}

#[tokio::test]
async fn replace_reenters_draft_and_bumps_version() {
    let base = spawn_app(true).await;
    let client = Client::new();

    // replace=true with no existing slug is refused (name-stable), never a
    // silent second plugin.
    let err = call_err(
        &client,
        &base,
        None,
        "plugins_create",
        json!({ "manifest": tool_manifest("Upgrader"), "wasm_b64": stub_wasm_b64(),
                "replace": true }),
    )
    .await;
    assert!(err.contains("name-stable"), "unhelpful: {err}");

    let created = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({ "manifest": tool_manifest("Upgrader"), "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["version_int"], 1);

    // Same slug without replace → conflict guidance.
    let err = call_err(
        &client,
        &base,
        None,
        "plugins_create",
        json!({ "manifest": tool_manifest("Upgrader"), "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    assert!(err.contains("replace=true"), "unhelpful: {err}");

    // Approve + enable, then replace: knocked back to draft, version bumped.
    client
        .post(format!("{base}/plugins/{id}/approve"))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base}/plugins/{id}/enable"))
        .send()
        .await
        .unwrap();

    let mut v2 = tool_manifest("Upgrader");
    v2["version"] = json!("0.2.0");
    let replaced = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({ "manifest": v2, "wasm_b64": stub_wasm_b64(), "replace": true }),
    )
    .await;
    assert_eq!(replaced["id"], id.as_str(), "same row, not a new one");
    assert_eq!(replaced["version_int"], 2);
    assert_eq!(replaced["status"], "draft");
    assert_eq!(replaced["review"], "draft");

    // The replaced plugin must no longer be live.
    let listed: Value = client
        .post(format!("{base}/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"].as_str().unwrap().starts_with("plugin__")));
}

#[tokio::test]
async fn bundle_digest_is_pinned_and_exported() {
    // No API mutates a stored bundle out-of-band (by design — replace re-pins
    // and re-drafts), so tampering isn't reachable over HTTP; what this test
    // pins is that the digest is computed, stored, and round-trips through
    // export so a client CAN verify an artifact. (The registry's load-time
    // digest refusal is covered by a direct-DB corruption test in
    // `plugins_runtime.rs`, where load is actually exercised.)
    let base = spawn_app(true).await;
    let client = Client::new();

    let created = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({ "manifest": tool_manifest("Pinned"), "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    let exported = call_ok(
        &client,
        &base,
        "plugins_export",
        json!({ "slug": "pinned" }),
    )
    .await;
    assert_eq!(exported["sha256"], created["bundle_sha256"]);
    assert_eq!(exported["wasm_b64"], stub_wasm_b64());
    assert_eq!(exported["sha256"].as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn run_actor_can_propose_but_not_uninstall() {
    let base = spawn_app(true).await;
    let client = Client::new();

    // A Baitler-spawned run (X-Baitler-Run) CAN author + propose…
    let body = call_raw(
        &client,
        &base,
        Some("run-123"),
        "plugins_create",
        json!({ "manifest": tool_manifest("Run Authored"), "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    assert_eq!(body["result"]["isError"], false, "{body}");
    let created: Value =
        serde_json::from_str(body["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(created["status"], "draft");
    assert_eq!(created["run_id"], "run-123");

    // …and the provenance lands in the activity log under that run.
    let activity = call_ok(
        &client,
        &base,
        "activity_list",
        json!({ "run_id": "run-123" }),
    )
    .await;
    let entries = activity["activity"].as_array().unwrap();
    assert!(entries
        .iter()
        .any(|a| a["action"] == "plugin.create" && a["run_id"] == "run-123"));

    // …but uninstall is refused BEFORE dispatch, like workspace_*.
    let err = call_err(
        &client,
        &base,
        Some("run-123"),
        "plugins_uninstall",
        json!({ "slug": "run-authored" }),
    )
    .await;
    assert!(
        err.contains("not available to Baitler-spawned agent runs"),
        "unhelpful: {err}"
    );

    // A run may re-author its own DRAFT via replace…
    let body = call_raw(
        &client,
        &base,
        Some("run-123"),
        "plugins_create",
        json!({ "manifest": tool_manifest("Run Authored"), "wasm_b64": stub_wasm_b64(),
                "replace": true }),
    )
    .await;
    assert_eq!(body["result"]["isError"], false, "{body}");

    // …but once a human approves it, a run can no longer swap the bundle.
    let created: Value =
        serde_json::from_str(body["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let id = created["id"].as_str().unwrap();
    client
        .post(format!("{base}/plugins/{id}/approve"))
        .send()
        .await
        .unwrap();
    let err = call_err(
        &client,
        &base,
        Some("run-123"),
        "plugins_create",
        json!({ "manifest": tool_manifest("Run Authored"), "wasm_b64": stub_wasm_b64(),
                "replace": true }),
    )
    .await;
    assert!(err.contains("human-approved"), "unhelpful: {err}");

    // No approve/enable verb exists on the MCP surface AT ALL.
    let body = call_raw(&client, &base, None, "plugins_approve", json!({})).await;
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));
    let body = call_raw(&client, &base, None, "plugins_enable", json!({})).await;
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));
}

fn ui_manifest(name: &str) -> Value {
    json!({
        "name": name,
        "version": "0.1.0",
        "description": "ui widget",
        "ui": [{ "slot": "detail", "key": "w", "label": "Widget", "entry": "ui/index.html" }],
        "capabilities": {}
    })
}

#[tokio::test]
async fn ui_assets_are_validated_served_opaque_origin_and_exported() {
    let base = spawn_app(true).await;
    let client = Client::new();

    // Declaring ui[] without supplying the assets is refused with guidance.
    let err = call_err(
        &client,
        &base,
        None,
        "plugins_create",
        json!({ "manifest": ui_manifest("UI Widget") }),
    )
    .await;
    assert!(err.contains("ui_assets"), "unhelpful: {err}");

    // Traversal-y asset paths are refused at upload.
    let err = call_err(
        &client,
        &base,
        None,
        "plugins_create",
        json!({ "manifest": ui_manifest("UI Widget"),
                "ui_assets": { "ui/../index.html": base64(b"x") } }),
    )
    .await;
    assert!(err.contains("invalid"), "unhelpful: {err}");

    // A UI-only plugin needs no WASM at all.
    let html = b"<!doctype html><html><head><script src=\"app.js\"></script></head>\
                 <body>widget</body></html>";
    let created = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({ "manifest": ui_manifest("UI Widget"), "ui_assets": {
            "ui/index.html": base64(html),
            "ui/app.js": base64(b"console.log('widget')"),
        } }),
    )
    .await;
    assert_eq!(created["status"], "draft");
    assert_eq!(created["kinds"], json!(["ui_component"]));
    let id = created["id"].as_str().unwrap().to_string();

    // Drafts are NOT served (404, existence unconfirmed).
    let resp = client
        .get(format!("{base}/plugin-ui/{id}/ui/index.html"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Approve + enable → served from the opaque, cookieless origin.
    client
        .post(format!("{base}/plugins/{id}/approve"))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base}/plugins/{id}/enable"))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{base}/plugin-ui/{id}/ui/index.html"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let csp = resp
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(csp.contains("sandbox allow-scripts"), "csp: {csp}");
    assert!(!csp.contains("allow-same-origin"), "csp: {csp}");
    assert!(csp.contains("script-src 'self'"), "csp: {csp}");
    assert!(csp.contains("connect-src 'none'"), "csp: {csp}");
    assert!(
        csp.contains("frame-ancestors http://localhost:8100"),
        "csp: {csp}"
    );
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let body = resp.text().await.unwrap();
    assert!(body.contains("widget"));

    // Sibling assets serve with their own type; unknown paths 404; encoded
    // traversal is rejected at the boundary.
    let resp = client
        .get(format!("{base}/plugin-ui/{id}/ui/app.js"))
        .send()
        .await
        .unwrap();
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/javascript"));
    let resp = client
        .get(format!("{base}/plugin-ui/{id}/ui/nope.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = client
        .get(format!("{base}/plugin-ui/{id}/ui/%2e%2e/secret"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Export round-trips the assets (backup/transfer).
    let exported = call_ok(
        &client,
        &base,
        "plugins_export",
        json!({ "slug": "ui-widget" }),
    )
    .await;
    assert_eq!(
        exported["ui_assets"]["ui/app.js"],
        base64(b"console.log('widget')")
    );

    // Disable de-serves.
    client
        .post(format!("{base}/plugins/{id}/disable"))
        .send()
        .await
        .unwrap();
    let resp = client
        .get(format!("{base}/plugin-ui/{id}/ui/index.html"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn uninstall_scrubs_links_and_kv() {
    let base = spawn_app(true).await;
    let client = Client::new();

    let created = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({ "manifest": tool_manifest("Linked Plugin"), "wasm_b64": stub_wasm_b64() }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Link it to an idea in the knowledge graph (plugins are first-class items).
    let idea = call_ok(
        &client,
        &base,
        "ideas_create",
        json!({ "title": "uses the plugin", "body": "x" }),
    )
    .await;
    let idea_id = idea["id"].as_str().unwrap().to_string();
    call_ok(
        &client,
        &base,
        "knowledge_link",
        json!({ "src_type": "plugin", "src_id": id, "dst_type": "idea", "dst_id": idea_id }),
    )
    .await;
    let backlinks = call_ok(
        &client,
        &base,
        "knowledge_backlinks",
        json!({ "item_type": "idea", "item_id": idea_id }),
    )
    .await;
    assert_eq!(backlinks["links"].as_array().unwrap().len(), 1);

    // Uninstall (no run header) deletes + scrubs.
    let gone = call_ok(
        &client,
        &base,
        "plugins_uninstall",
        json!({ "slug": "linked-plugin" }),
    )
    .await;
    assert_eq!(gone["deleted"], true);

    let backlinks = call_ok(
        &client,
        &base,
        "knowledge_backlinks",
        json!({ "item_type": "idea", "item_id": idea_id }),
    )
    .await;
    assert_eq!(backlinks["links"].as_array().unwrap().len(), 0);

    let err = call_err(
        &client,
        &base,
        None,
        "plugins_get",
        json!({ "slug": "linked-plugin" }),
    )
    .await;
    assert!(err.contains("not found"));
}
