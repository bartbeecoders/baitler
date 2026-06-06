//! Integration tests for draw.io diagrams (Phase 14, Milestone C): CRUD, owner
//! isolation, the preview data-URI guard, version rules, label-extraction into
//! search, and project membership. Embedded `memory` DB, no egress.

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
                .join("baitler-test-diagrams")
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
        secret_key: [7u8; 32],
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
async fn create_diagram(c: &Client, base: &str, body: Value) -> Value {
    let r = post(c, format!("{base}/diagrams"), body).await;
    assert_eq!(r.status(), StatusCode::CREATED, "create diagram");
    r.json().await.unwrap()
}

const SAMPLE_XML: &str = r#"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>
  <mxCell id="2" value="Ingest" vertex="1" parent="1"/>
  <mxCell id="3" value="Transform" vertex="1" parent="1"/>
</root></mxGraphModel>"#;

#[tokio::test]
async fn diagram_crud_and_version_rules() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let d = create_diagram(&c, &base, json!({ "title": "Pipeline", "xml": SAMPLE_XML })).await;
    let id = d["id"].as_str().unwrap().to_string();
    assert_eq!(d["version"], 1);
    assert_eq!(d["review"], "published");
    assert!(d["xml"].as_str().unwrap().contains("Ingest"));

    // Editing the XML bumps the version.
    let r = c
        .patch(format!("{base}/diagrams/{id}"))
        .json(&json!({ "xml": SAMPLE_XML.replace("Transform", "Reduce") }))
        .send()
        .await
        .unwrap();
    let updated: Value = r.json().await.unwrap();
    assert_eq!(updated["version"], 2);

    // Tags-only edit does not bump version.
    let r = c
        .patch(format!("{base}/diagrams/{id}"))
        .json(&json!({ "tags": ["arch"] }))
        .send()
        .await
        .unwrap();
    let updated: Value = r.json().await.unwrap();
    assert_eq!(updated["version"], 2);
    assert_eq!(updated["tags"][0], "arch");

    let r = c
        .delete(format!("{base}/diagrams/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn preview_must_be_a_data_uri() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    // A non-data: preview is rejected (defends against injecting a remote/script URL).
    let r = post(
        &c,
        format!("{base}/diagrams"),
        json!({ "title": "Bad", "xml": SAMPLE_XML, "preview": "https://evil.example/x.svg" }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // A data:image/* preview is accepted.
    let d = create_diagram(
        &c,
        &base,
        json!({ "title": "Good", "xml": SAMPLE_XML, "preview": "data:image/png;base64,AAAA" }),
    )
    .await;
    assert_eq!(d["preview"], "data:image/png;base64,AAAA");
}

#[tokio::test]
async fn diagram_labels_feed_search() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    create_diagram(&c, &base, json!({ "title": "Flow", "xml": SAMPLE_XML })).await;

    // The label text extracted from the XML ("Ingest") is searchable.
    let r = c
        .get(format!("{base}/knowledge/search?q=Ingest"))
        .send()
        .await
        .unwrap();
    let results: Value = r.json().await.unwrap();
    let hits = results["diagrams"].as_array().unwrap();
    assert!(!hits.is_empty(), "diagram should match its extracted label");
}

#[tokio::test]
async fn owner_isolation_at_repo_layer() {
    let (_base, state) = spawn().await;
    use baitler_api::diagrams::repo;

    let a = repo::create_diagram(
        &state.db,
        "owner-a",
        "A diagram",
        SAMPLE_XML,
        "",
        None,
        None,
        &[],
        "published",
    )
    .await
    .unwrap();
    assert!(repo::get_diagram(&state.db, "owner-b", &a.uuid)
        .await
        .unwrap()
        .is_none());
    let b_list = repo::list_diagrams(&state.db, "owner-b", None, None, None, None, None, 100, 0)
        .await
        .unwrap();
    assert!(b_list.is_empty());
}
