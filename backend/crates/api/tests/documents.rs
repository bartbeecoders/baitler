//! Integration tests for documents + the export pathway. PDF export uses
//! headless Chrome (present in this environment); DOCX needs Pandoc (absent →
//! expected 503).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{Config, McpConfig, StorageConfig, SurrealConfig};
use baitler_api::{documents::repo, AppState};
use reqwest::Client;
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
                .join("baitler-test-docs")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp: McpConfig {
            enabled: true,
            auth_token: None,
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

async fn create(client: &Client, base: &str, payload: Value) -> Value {
    let resp = client
        .post(format!("{base}/documents"))
        .json(&payload)
        .send()
        .await
        .expect("create");
    assert_eq!(resp.status(), 201);
    resp.json().await.unwrap()
}

#[tokio::test]
async fn document_crud_sanitizes_and_versions() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // Body with a script tag is sanitized on store.
    let doc = create(
        &client,
        &base,
        json!({ "title": "Doc", "body": "<h1>Hi</h1><script>alert(1)</script>" }),
    )
    .await;
    let id = doc["id"].as_str().unwrap().to_string();
    assert!(!doc["body"].as_str().unwrap().contains("script"));
    assert_eq!(doc["version"], 1);

    // Patch bumps the version.
    let patched: Value = client
        .patch(format!("{base}/documents/{id}"))
        .json(&json!({ "body": "<p>updated</p>" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(patched["version"], 2);

    // Delete then 404.
    let del = client
        .delete(format!("{base}/documents/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);
    let after = client
        .get(format!("{base}/documents/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), 404);
}

#[tokio::test]
async fn export_to_markdown_and_html() {
    let (base, _state) = spawn().await;
    let client = Client::new();
    let doc = create(
        &client,
        &base,
        json!({ "title": "Doc", "body": "<h1>Title</h1><p>hello <strong>world</strong></p>" }),
    )
    .await;
    let id = doc["id"].as_str().unwrap();

    let md = client
        .get(format!("{base}/documents/{id}/export?format=markdown"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(md.contains("Title"));
    assert!(md.contains("**world**"));

    // Shared POST /export: markdown -> html.
    let resp = client
        .post(format!("{base}/export"))
        .json(&json!({ "content": "# Heading\n\n- a", "source": "markdown", "target": "html" }))
        .send()
        .await
        .unwrap();
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let html = resp.text().await.unwrap();
    assert!(html.contains("<h1>Heading</h1>"));
    assert!(html.contains("<li>a</li>"));
}

#[tokio::test]
async fn export_to_pdf_via_chrome() {
    let (base, _state) = spawn().await;
    let client = Client::new();
    let doc = create(
        &client,
        &base,
        json!({ "title": "Report", "body": "<h1>Quarterly</h1><p>Numbers.</p>" }),
    )
    .await;
    let id = doc["id"].as_str().unwrap();

    let resp = client
        .get(format!("{base}/documents/{id}/export?format=pdf"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/pdf"
    );
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF"), "expected a PDF document");
}

#[tokio::test]
async fn export_errors() {
    let (base, _state) = spawn().await;
    let client = Client::new();
    let doc = create(
        &client,
        &base,
        json!({ "title": "Doc", "body": "<p>x</p>" }),
    )
    .await;
    let id = doc["id"].as_str().unwrap();

    // Unsupported format → 400.
    let bad = client
        .get(format!("{base}/documents/{id}/export?format=xlsx"))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);

    // DOCX needs Pandoc, which isn't installed → 503.
    let docx = client
        .get(format!("{base}/documents/{id}/export?format=docx"))
        .send()
        .await
        .unwrap();
    assert_eq!(docx.status(), 503);
}

#[tokio::test]
async fn documents_are_owner_scoped() {
    let (_base, state) = spawn().await;
    let db = &state.db;

    let alice = repo::create_document(db, "alice", "A", "<p>a</p>", "published", None)
        .await
        .unwrap();
    repo::create_document(db, "bob", "B", "<p>b</p>", "published", None)
        .await
        .unwrap();

    let alice_docs = repo::list_documents(db, "alice").await.unwrap();
    assert_eq!(alice_docs.len(), 1);
    assert_eq!(alice_docs[0].uuid, alice.uuid);
    assert!(repo::get_document(db, "bob", &alice.uuid)
        .await
        .unwrap()
        .is_none());
}
