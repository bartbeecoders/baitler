//! Integration tests for superpages (Phase 15).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{CliConfig, Config, McpConfig, StorageConfig, SurrealConfig};
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
                .join("baitler-test-superpages")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp: McpConfig {
            enabled: true,
            auth_token: None,
        },
        cli: CliConfig::default(),
        workspace_roots: Vec::new(),
        plugins: Default::default(),
        public_page_origin: None,
        secret_key: [9u8; 32],
    }
}

async fn spawn() -> (String, AppState) {
    let state = baitler_api::build_state(test_config())
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

fn id_of(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn superpage_crud_context_and_from_project() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let idea = post(
        &c,
        format!("{base}/ideas"),
        json!({ "title": "Board idea", "body": "hello" }),
    )
    .await;
    assert_eq!(idea.status(), StatusCode::CREATED);
    let idea_id = id_of(&idea.json().await.unwrap());

    let sp = post(
        &c,
        format!("{base}/superpages"),
        json!({
            "title": "My board",
            "blocks": {
                "layout": "grid",
                "blocks": [
                    { "id": "b1", "kind": "heading", "text": "Focus" },
                    { "id": "b2", "kind": "embed", "item_type": "idea", "item_id": idea_id },
                    { "id": "b3", "kind": "note", "markdown": "Agent: summarize the idea." }
                ]
            }
        }),
    )
    .await;
    assert_eq!(sp.status(), StatusCode::CREATED);
    let sp_body: Value = sp.json().await.unwrap();
    let sp_id = id_of(&sp_body);
    assert_eq!(sp_body["blocks"]["blocks"].as_array().unwrap().len(), 3);

    let ctx = c
        .get(format!("{base}/superpages/{sp_id}/context"))
        .send()
        .await
        .unwrap();
    assert_eq!(ctx.status(), StatusCode::OK);
    let ctx_body: Value = ctx.json().await.unwrap();
    assert_eq!(ctx_body["blocks"].as_array().unwrap().len(), 3);
    let embed = &ctx_body["blocks"][1];
    assert_eq!(embed["kind"], "embed");
    assert_eq!(embed["title"], "Board idea");

    let proj = post(&c, format!("{base}/projects"), json!({ "name": "Sprint" })).await;
    assert_eq!(proj.status(), StatusCode::CREATED);
    let proj_id = id_of(&proj.json().await.unwrap());

    let seeded = post(
        &c,
        format!("{base}/superpages/from-project"),
        json!({ "project_id": proj_id }),
    )
    .await;
    assert_eq!(seeded.status(), StatusCode::CREATED);
    let seeded_body: Value = seeded.json().await.unwrap();
    assert!(seeded_body["title"].as_str().unwrap().contains("Sprint"));

    let del = c
        .delete(format!("{base}/superpages/{sp_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn superpage_typed_parts_resolve_in_context() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let sp = post(
        &c,
        format!("{base}/superpages"),
        json!({
            "title": "Canvas",
            "blocks": {
                "layout": "canvas",
                "blocks": [
                    { "id": "t1", "kind": "text", "markdown": "# Hi\nsome notes", "x": 0, "y": 0, "w": 360, "h": 200 },
                    { "id": "c1", "kind": "code", "lang": "rust", "text": "fn main() {}", "x": 0, "y": 220, "w": 360, "h": 200 },
                    { "id": "i1", "kind": "image", "src": "url", "url": "https://example.com/a.png", "x": 380, "y": 0, "w": 360, "h": 240 },
                    { "id": "w1", "kind": "webpage", "web_kind": "url", "url": "https://example.com", "x": 380, "y": 260, "w": 360, "h": 240 }
                ]
            }
        }),
    )
    .await;
    assert_eq!(sp.status(), StatusCode::CREATED);
    let sp_id = id_of(&sp.json().await.unwrap());

    let ctx = c
        .get(format!("{base}/superpages/{sp_id}/context"))
        .send()
        .await
        .unwrap();
    let body: Value = ctx.json().await.unwrap();
    let blocks = body["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 4);
    assert_eq!(blocks[0]["kind"], "text");
    assert_eq!(blocks[1]["kind"], "code");
    assert_eq!(blocks[1]["lang"], "rust");
    assert_eq!(blocks[2]["kind"], "image");
    assert_eq!(blocks[2]["url"], "https://example.com/a.png");
    assert_eq!(blocks[3]["kind"], "webpage");
    assert_eq!(blocks[3]["web_kind"], "url");
}

#[tokio::test]
async fn superpage_rejects_bad_webpage_url() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    let r = post(
        &c,
        format!("{base}/superpages"),
        json!({
            "title": "Bad",
            "blocks": { "blocks": [
                { "id": "w1", "kind": "webpage", "web_kind": "url", "url": "javascript:alert(1)" }
            ] }
        }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn superpage_rejects_bad_embed() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    let r = post(
        &c,
        format!("{base}/superpages"),
        json!({
            "title": "Bad",
            "blocks": {
                "blocks": [{ "id": "b1", "kind": "embed", "item_type": "idea", "item_id": "nope" }]
            }
        }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
