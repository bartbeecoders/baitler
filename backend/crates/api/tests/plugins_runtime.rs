//! WASM-execution tests for the plugin system (Phase 16.B) — compiled only
//! with `--features plugins` (CI runs both configurations).
//!
//! Guests are hand-written WAT fixtures (compiled in-test by the `wat` crate;
//! no PDK toolchain, no egress): `echo` proves the kernel ABI + dispatch end
//! to end, `spin` proves the epoch-timeout kill, and `kv` proves host-fn
//! grants (structural containment: an ungranted import fails instantiation)
//! plus persistent-vs-throwaway storage semantics.

#![cfg(feature = "plugins")]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{
    CliConfig, Config, McpConfig, PluginsConfig, StorageConfig, SurrealConfig,
};
use baitler_api::AppState;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;

fn test_config() -> Config {
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
                .join("baitler-test-plugin-rt-files")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp: McpConfig {
            enabled: true,
            auth_token: None,
        },
        cli: CliConfig::default(),
        plugins: PluginsConfig { enabled: true },
        workspace_roots: Vec::new(),
        public_page_origin: None,
        secret_key: [7u8; 32],
    }
}

/// Boot an app and ALSO hand back its state (for direct-DB corruption tests).
async fn spawn_app() -> (String, AppState) {
    let state = baitler_api::build_state(test_config())
        .await
        .expect("failed to build app state");
    let app = baitler_api::build_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server error");
    });
    (format!("http://{addr}"), state)
}

async fn call_raw(client: &Client, base: &str, name: &str, args: Value) -> Value {
    client
        .post(format!("{base}/mcp"))
        .header("X-Baitler-Agent", "rt-test")
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": args }
        }))
        .send()
        .await
        .expect("request failed")
        .json()
        .await
        .expect("json body")
}

async fn call_ok(client: &Client, base: &str, name: &str, args: Value) -> Value {
    let body = call_raw(client, base, name, args).await;
    assert_eq!(
        body["result"]["isError"], false,
        "tool `{name}` errored: {body}"
    );
    serde_json::from_str(body["result"]["content"][0]["text"].as_str().unwrap())
        .expect("tool result was not JSON")
}

async fn call_err(client: &Client, base: &str, name: &str, args: Value) -> String {
    let body = call_raw(client, base, name, args).await;
    assert_eq!(
        body["result"]["isError"], true,
        "tool `{name}` unexpectedly succeeded: {body}"
    );
    body["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

fn b64(bytes: &[u8]) -> String {
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

fn wasm_b64(wat_source: &str) -> String {
    b64(&wat::parse_str(wat_source).expect("fixture WAT compiles"))
}

fn echo_wasm() -> String {
    wasm_b64(include_str!("fixtures/echo.wat"))
}

fn spin_wasm() -> String {
    wasm_b64(include_str!("fixtures/spin.wat"))
}

fn kv_wasm() -> String {
    wasm_b64(include_str!("fixtures/kv.wat"))
}

/// Create + approve + enable in one go; returns (id, slug).
async fn install(client: &Client, base: &str, manifest: Value, wasm: String) -> (String, String) {
    let created = call_ok(
        client,
        base,
        "plugins_create",
        json!({ "manifest": manifest, "wasm_b64": wasm }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    let slug = created["slug"].as_str().unwrap().to_string();
    let r = client
        .post(format!("{base}/plugins/{id}/approve"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let r = client
        .post(format!("{base}/plugins/{id}/enable"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK, "{}", r.text().await.unwrap());
    (id, slug)
}

#[tokio::test]
async fn echo_plugin_executes_end_to_end() {
    let (base, _state) = spawn_app().await;
    let client = Client::new();

    let manifest = json!({
        "name": "Echo Plugin",
        "version": "1.0.0",
        "description": "echoes its input",
        "tools": [{ "name": "echo", "export": "echo", "description": "echo back",
                    "input_schema": { "type": "object", "properties": {}, "required": [] } }],
        "capabilities": {}
    });
    let (_id, slug) = install(&client, &base, manifest, echo_wasm()).await;

    // The namespaced tool path (what a model picks from tools/list)…
    let out = call_ok(
        &client,
        &base,
        &format!("plugin__{slug}__echo"),
        json!({ "hello": "world", "n": 7 }),
    )
    .await;
    assert_eq!(out, json!({ "hello": "world", "n": 7 }));

    // …and the meta-tool path behave identically.
    let out = call_ok(
        &client,
        &base,
        "plugins_invoke",
        json!({ "slug": slug, "tool": "echo", "input": { "via": "invoke" } }),
    )
    .await;
    assert_eq!(out, json!({ "via": "invoke" }));

    // Every invocation logs an attributed provenance row — BOTH paths: the
    // namespaced tool (titled with the full plugin__ name) and the meta-tool.
    let activity = call_ok(&client, &base, "activity_list", json!({})).await;
    let invokes: Vec<&Value> = activity["activity"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["action"] == "plugin.invoke")
        .collect();
    assert_eq!(invokes.len(), 2, "expected plugin.invoke rows: {activity}");
    assert!(invokes
        .iter()
        .any(|a| a["target_title"] == format!("plugin__{slug}__echo")));
    assert!(invokes.iter().all(|a| a["agent"] == "rt-test"));
}

#[tokio::test]
async fn spinning_guest_is_killed_by_the_timeout() {
    let (base, _state) = spawn_app().await;
    let client = Client::new();

    let manifest = json!({
        "name": "Spinner",
        "tools": [{ "name": "spin", "export": "spin" }],
        "capabilities": { "timeout_ms": 300 }
    });
    let (_id, slug) = install(&client, &base, manifest, spin_wasm()).await;

    let started = std::time::Instant::now();
    let err = call_err(&client, &base, &format!("plugin__{slug}__spin"), json!({})).await;
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "kill took too long"
    );
    assert!(
        err.contains("resource limit"),
        "expected a limit kill, got: {err}"
    );
}

#[tokio::test]
async fn kv_host_fns_persist_across_invocations_and_uninstall_cascades() {
    let (base, state) = spawn_app().await;
    let client = Client::new();

    let manifest = json!({
        "name": "KV Plugin",
        "tools": [
            { "name": "set", "export": "set" },
            { "name": "get", "export": "get" }
        ],
        "capabilities": { "host_fns": ["plugin_kv_get", "plugin_kv_set"] }
    });
    let (_id, slug) = install(&client, &base, manifest, kv_wasm()).await;

    call_ok(
        &client,
        &base,
        &format!("plugin__{slug}__set"),
        json!({ "k": "counter", "v": { "n": 41 } }),
    )
    .await;
    // A separate invocation = a FRESH instance; only plugin_kv persists.
    let got = call_ok(
        &client,
        &base,
        &format!("plugin__{slug}__get"),
        json!({ "k": "counter" }),
    )
    .await;
    assert_eq!(got, json!({ "v": { "n": 41 } }));

    // Uninstall cascades the kv rows (checked at the DB, the only place left).
    call_ok(&client, &base, "plugins_uninstall", json!({ "slug": slug })).await;
    let mut res = state
        .db
        .query("SELECT VALUE k FROM plugin_kv WHERE slug = $slug")
        .bind(("slug", slug.clone()))
        .await
        .unwrap();
    let leftover: Vec<String> = res.take(0).unwrap();
    assert!(leftover.is_empty(), "kv rows survived uninstall");
}

#[tokio::test]
async fn ungranted_host_import_fails_at_instantiation() {
    let (base, _state) = spawn_app().await;
    let client = Client::new();

    // Same kv guest, but the manifest grants NOTHING: the guest's host-fn
    // imports cannot link, so the call dies before any guest code runs.
    let manifest = json!({
        "name": "Greedy Plugin",
        "tools": [{ "name": "set", "export": "set" }],
        "capabilities": { "host_fns": [] }
    });
    let (_id, slug) = install(&client, &base, manifest, kv_wasm()).await;

    let err = call_err(
        &client,
        &base,
        &format!("plugin__{slug}__set"),
        json!({ "k": "x", "v": 1 }),
    )
    .await;
    assert!(
        err.contains("instantiation failed"),
        "expected a link failure, got: {err}"
    );
}

#[tokio::test]
async fn plugins_test_sandbox_is_throwaway() {
    let (base, _state) = spawn_app().await;
    let client = Client::new();

    let manifest = json!({
        "name": "Sandboxed",
        "tools": [
            { "name": "set", "export": "set" },
            { "name": "get", "export": "get" }
        ],
        "capabilities": { "host_fns": ["plugin_kv_get", "plugin_kv_set"] }
    });

    // A set in one plugins_test call…
    let set = call_ok(
        &client,
        &base,
        "plugins_test",
        json!({ "manifest": manifest, "wasm_b64": kv_wasm(), "export": "set",
                "input": { "k": "a", "v": 1 } }),
    )
    .await;
    assert_eq!(set["ok"], true, "{set}");

    // …is INVISIBLE to the next one (fresh in-memory kv per call), and nothing
    // was installed.
    let get = call_ok(
        &client,
        &base,
        "plugins_test",
        json!({ "manifest": manifest, "wasm_b64": kv_wasm(), "export": "get",
                "input": { "k": "a" } }),
    )
    .await;
    assert_eq!(get["ok"], true, "{get}");
    assert_eq!(get["output"], json!({ "v": null }));

    let listed = call_ok(&client, &base, "plugins_list", json!({})).await;
    assert!(listed["plugins"].as_array().unwrap().is_empty());

    // A guest failure comes back as DATA (ok:false), not a tool error.
    let failed = call_ok(
        &client,
        &base,
        "plugins_test",
        json!({ "manifest": manifest, "wasm_b64": kv_wasm(), "export": "missing_export",
                "input": {} }),
    )
    .await;
    assert_eq!(failed["ok"], false);
}

#[tokio::test]
async fn corrupted_bundle_refuses_enable_and_rolls_back_to_disabled() {
    let (base, state) = spawn_app().await;
    let client = Client::new();

    let manifest = json!({
        "name": "Tampered",
        "tools": [{ "name": "echo", "export": "echo" }],
        "capabilities": {}
    });
    let created = call_ok(
        &client,
        &base,
        "plugins_create",
        json!({ "manifest": manifest, "wasm_b64": echo_wasm() }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    let r = client
        .post(format!("{base}/plugins/{id}/approve"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Corrupt the stored artifact out-of-band: the pinned sha256 no longer
    // matches what install computed.
    state
        .db
        .query("UPDATE plugin SET wasm_b64 = $w WHERE uuid = $id")
        .bind(("w", spin_wasm()))
        .bind(("id", id.clone()))
        .await
        .unwrap();

    let r = client
        .post(format!("{base}/plugins/{id}/enable"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT, "digest must refuse");
    let body = r.text().await.unwrap();
    assert!(body.contains("digest mismatch"), "{body}");

    // The row never claims to be running: rolled back to disabled.
    let row: Value = client
        .get(format!("{base}/plugins/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(row["status"], "disabled");

    // And nothing got advertised.
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
