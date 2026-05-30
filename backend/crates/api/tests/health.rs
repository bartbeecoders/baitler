//! End-to-end integration tests.
//!
//! Each test boots a full application instance — embedded in-memory SurrealDB,
//! migrations applied, real Axum router — on an ephemeral TCP port, then drives
//! it over real HTTP with `reqwest`. The `memory` engine gives every test its
//! own isolated datastore, so tests are independent and need no external server.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{Config, McpConfig, StorageConfig, SurrealConfig};
use baitler_api::AppState;
use reqwest::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD,
    CONTENT_TYPE, ORIGIN,
};
use reqwest::{Client, Method, StatusCode};
use tokio::net::TcpListener;

const ALLOWED_ORIGIN: &str = "http://localhost:8100";

/// Build a `Config` pointing at an isolated in-memory datastore.
fn test_config() -> Config {
    Config {
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        cors_allowed_origins: vec![ALLOWED_ORIGIN.to_string()],
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
                .join("baitler-test-files")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp: McpConfig {
            enabled: true,
            auth_token: None,
        },
        public_page_origin: None,
        secret_key: [7u8; 32],
    }
}

async fn build_test_state() -> AppState {
    baitler_api::build_state(test_config())
        .await
        .expect("failed to build app state")
}

/// Boot an app instance on a random port and return its base URL.
async fn spawn_app() -> String {
    let app = baitler_api::build_app(build_test_state().await);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind ephemeral port");
    let addr = listener.local_addr().expect("failed to read local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server error");
    });

    format!("http://{addr}")
}

fn assert_json_content_type(resp: &reqwest::Response) {
    let ct = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        ct.starts_with("application/json"),
        "expected JSON content-type, got {ct:?}"
    );
}

#[tokio::test]
async fn health_reports_ok() {
    let base = spawn_app().await;

    let resp = reqwest::get(format!("{base}/health"))
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_json_content_type(&resp);

    let body: serde_json::Value = resp.json().await.expect("invalid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], "up");
}

#[tokio::test]
async fn version_returns_crate_metadata() {
    let base = spawn_app().await;

    let resp = reqwest::get(format!("{base}/version"))
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_json_content_type(&resp);

    let body: serde_json::Value = resp.json().await.expect("invalid json");
    assert_eq!(body["name"], "baitler-api");
    assert!(
        body["version"].as_str().is_some_and(|v| !v.is_empty()),
        "version should be a non-empty string"
    );
}

#[tokio::test]
async fn unknown_route_returns_json_error_envelope() {
    let base = spawn_app().await;

    let resp = reqwest::get(format!("{base}/does-not-exist"))
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_json_content_type(&resp);

    let body: serde_json::Value = resp.json().await.expect("invalid json");
    assert_eq!(body["error"]["code"], "not_found");
    assert!(
        body["error"]["message"]
            .as_str()
            .is_some_and(|m| !m.is_empty()),
        "error message should be present and non-empty"
    );
    // The envelope is the public contract: exactly { code, message }.
    let detail = body["error"]
        .as_object()
        .expect("error should be an object");
    assert_eq!(detail.len(), 2);
    assert!(detail.contains_key("code") && detail.contains_key("message"));
}

#[tokio::test]
async fn cors_preflight_allows_configured_origin() {
    let base = spawn_app().await;

    let resp = Client::new()
        .request(Method::OPTIONS, format!("{base}/health"))
        .header(ORIGIN, ALLOWED_ORIGIN)
        .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(ACCESS_CONTROL_REQUEST_HEADERS, "authorization")
        .send()
        .await
        .expect("preflight failed");

    let headers = resp.headers();
    assert_eq!(
        headers
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some(ALLOWED_ORIGIN),
        "preflight should echo the configured origin, not a wildcard"
    );
    assert_eq!(
        headers
            .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .and_then(|v| v.to_str().ok()),
        Some("true")
    );
    let allow_methods = headers
        .get(ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        allow_methods.contains("POST"),
        "got methods: {allow_methods}"
    );
    let allow_headers = headers
        .get(ACCESS_CONTROL_ALLOW_HEADERS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(
        allow_headers.contains("authorization"),
        "got headers: {allow_headers}"
    );
}

#[tokio::test]
async fn cors_does_not_allow_unconfigured_origin() {
    let base = spawn_app().await;

    let resp = Client::new()
        .get(format!("{base}/health"))
        .header(ORIGIN, "http://evil.example")
        .send()
        .await
        .expect("request failed");

    // The request itself succeeds (CORS is browser-enforced), but the server
    // must not grant the disallowed origin.
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none(),
        "disallowed origin should not be echoed"
    );
}

#[tokio::test]
async fn migrations_are_idempotent() {
    // build_state runs migrations once; run twice more and confirm no dupes.
    let state = build_test_state().await;
    baitler_api::migrations::run(&state.db)
        .await
        .expect("second migration run");
    baitler_api::migrations::run(&state.db)
        .await
        .expect("third migration run");

    let mut applied = state
        .db
        .query("SELECT VALUE name FROM _migration")
        .await
        .expect("query _migration");
    let names: Vec<String> = applied.take(0).expect("take names");
    // Idempotent: each migration is recorded exactly once regardless of how many exist.
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        names.len(),
        "duplicate migration records: {names:?}"
    );
    for expected in ["0001_init", "0002_files", "0003_ideas"] {
        assert!(
            names.iter().any(|n| n.contains(expected)),
            "missing migration {expected}: {names:?}"
        );
    }

    let mut meta = state
        .db
        .query("SELECT VALUE schema_version FROM app_meta:current")
        .await
        .expect("query app_meta");
    let versions: Vec<i64> = meta.take(0).expect("take versions");
    assert_eq!(versions, vec![1], "schema_version should be set once to 1");
}
