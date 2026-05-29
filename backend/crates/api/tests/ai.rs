//! Integration tests for the AI feature (provider listing, key management, and
//! mock streaming chat). Real providers need keys + egress and aren't exercised.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{Config, StorageConfig, SurrealConfig};
use baitler_api::{ai::repo, AppState};
use reqwest::Client;
use serde_json::{json, Value};
use tokio::net::TcpListener;

fn test_config() -> Config {
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
                .join("baitler-test-ai")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        secret_key: [7u8; 32],
    }
}

async fn spawn() -> (String, AppState) {
    let state = baitler_api::build_state(test_config())
        .await
        .expect("build state");
    let app = baitler_api::build_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (format!("http://{addr}"), state)
}

#[tokio::test]
async fn providers_lists_mock_and_real() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    let body: Value = client
        .get(format!("{base}/ai/providers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let providers = body["providers"].as_array().unwrap();
    let mock = providers.iter().find(|p| p["id"] == "mock").unwrap();
    assert_eq!(mock["requires_key"], false);
    assert_eq!(mock["configured"], true); // mock needs no key
    let openai = providers.iter().find(|p| p["id"] == "openai").unwrap();
    assert_eq!(openai["requires_key"], true);
    assert_eq!(openai["configured"], false); // no key yet
}

#[tokio::test]
async fn mock_chat_streams_sse() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // The mock stream is finite, so .text() collects the whole SSE response.
    let body = client
        .post(format!("{base}/ai/chat"))
        .json(&json!({
            "provider": "mock",
            "model": "mock-1",
            "messages": [{ "role": "user", "content": "Hello there" }]
        }))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(body.contains("\"done\":true"), "expected a done event");

    // Reassemble the streamed deltas (each SSE `data:` line is a chunk).
    let text: String = body
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|p| serde_json::from_str::<Value>(p.trim()).ok())
        .filter_map(|v| v["delta"].as_str().map(str::to_string))
        .collect();
    assert!(text.contains("mock response"), "reassembled: {text}");
    assert!(
        text.contains("Hello there"),
        "mock echoes the user message: {text}"
    );
}

#[tokio::test]
async fn key_crud_and_configured_flag() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // Store a key for openai.
    let put = client
        .put(format!("{base}/ai/keys/openai"))
        .json(&json!({ "api_key": "sk-secret-123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 204);

    // It shows as configured (id only — never the key value).
    let keys: Value = client
        .get(format!("{base}/ai/keys"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(keys["configured"], json!(["openai"]));

    let providers: Value = client
        .get(format!("{base}/ai/providers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let openai = providers["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "openai")
        .unwrap();
    assert_eq!(openai["configured"], true);

    // Delete it.
    let del = client
        .delete(format!("{base}/ai/keys/openai"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);
    let keys2: Value = client
        .get(format!("{base}/ai/keys"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(keys2["configured"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn chat_validation() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // Unknown provider → 404.
    let unknown = client
        .post(format!("{base}/ai/chat"))
        .json(&json!({ "provider": "nope", "model": "x", "messages": [{ "role": "user", "content": "hi" }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), 404);

    // Empty messages → 400.
    let empty = client
        .post(format!("{base}/ai/chat"))
        .json(&json!({ "provider": "mock", "model": "mock-1", "messages": [] }))
        .send()
        .await
        .unwrap();
    assert_eq!(empty.status(), 400);

    // A real provider with no key configured → 400 (before any network call).
    let no_key = client
        .post(format!("{base}/ai/chat"))
        .json(&json!({ "provider": "openai", "model": "gpt-4o", "messages": [{ "role": "user", "content": "hi" }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(no_key.status(), 400);
}

#[tokio::test]
async fn keys_are_owner_scoped_and_encrypted() {
    let (_base, state) = spawn().await;
    let db = &state.db;

    repo::set_key(db, "alice", "openai", "cipher-A")
        .await
        .unwrap();
    repo::set_key(db, "bob", "openai", "cipher-B")
        .await
        .unwrap();

    assert_eq!(
        repo::get_ciphertext(db, "alice", "openai").await.unwrap(),
        Some("cipher-A".to_string())
    );
    assert_eq!(
        repo::get_ciphertext(db, "bob", "openai").await.unwrap(),
        Some("cipher-B".to_string())
    );
    assert!(repo::get_ciphertext(db, "alice", "anthropic")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        repo::list_configured(db, "alice").await.unwrap(),
        vec!["openai"]
    );
}
