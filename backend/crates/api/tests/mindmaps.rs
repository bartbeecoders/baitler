//! Integration tests for mindmaps (Phase 14, Milestone B): CRUD, owner
//! isolation, graph shape validation, the Markdown-outline + project seeds,
//! version rules, search fold, project membership, and `kn_link` scrub-on-delete.
//! All on an embedded `memory` DB, no egress.

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
                .join("baitler-test-mindmaps")
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
async fn create_mindmap(c: &Client, base: &str, body: Value) -> Value {
    let r = post(c, format!("{base}/mindmaps"), body).await;
    assert_eq!(r.status(), StatusCode::CREATED, "create mindmap");
    r.json().await.unwrap()
}
fn id_of(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn mindmap_crud_and_version_rules() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let mm = create_mindmap(
        &c,
        &base,
        json!({
            "title": "Plan",
            "graph": { "nodes": [{ "id": "a", "label": "Root" }], "edges": [] }
        }),
    )
    .await;
    let id = id_of(&mm);
    assert_eq!(mm["version"], 1);
    assert_eq!(mm["review"], "published"); // portal default
    assert_eq!(mm["graph"]["nodes"].as_array().unwrap().len(), 1);

    // A content edit (graph) bumps the version.
    let r = c
        .patch(format!("{base}/mindmaps/{id}"))
        .json(&json!({ "graph": { "nodes": [{ "id": "a", "label": "Root" }, { "id": "b", "label": "Child", "parent": "a" }], "edges": [{ "from": "a", "to": "b" }] } }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let updated: Value = r.json().await.unwrap();
    assert_eq!(updated["version"], 2);
    assert_eq!(updated["graph"]["nodes"].as_array().unwrap().len(), 2);

    // A tags-only edit does NOT bump the version.
    let r = c
        .patch(format!("{base}/mindmaps/{id}"))
        .json(&json!({ "tags": ["planning"] }))
        .send()
        .await
        .unwrap();
    let updated: Value = r.json().await.unwrap();
    assert_eq!(
        updated["version"], 2,
        "tags-only edit must not bump version"
    );
    assert_eq!(updated["tags"][0], "planning");

    // Delete.
    let r = c
        .delete(format!("{base}/mindmaps/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    let r = c.get(format!("{base}/mindmaps/{id}")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_dangling_edge() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    let r = post(
        &c,
        format!("{base}/mindmaps"),
        json!({
            "title": "Broken",
            "graph": { "nodes": [{ "id": "a", "label": "A" }], "edges": [{ "from": "a", "to": "ghost" }] }
        }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST, "edge to unknown node");
}

#[tokio::test]
async fn outline_seed_builds_a_tree() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    let mm = create_mindmap(
        &c,
        &base,
        json!({ "title": "Outline", "outline": "# Root\n- A\n  - A1\n- B" }),
    )
    .await;
    assert_eq!(mm["source_format"], "markdown");
    let nodes = mm["graph"]["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 4);
    let edges = mm["graph"]["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 3);
}

#[tokio::test]
async fn node_item_link_must_reference_owned_item() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    // A node pointing at a non-existent idea is rejected.
    let r = post(
        &c,
        format!("{base}/mindmaps"),
        json!({
            "title": "Linked",
            "graph": { "nodes": [{ "id": "a", "label": "A", "item_type": "idea", "item_id": "nope" }], "edges": [] }
        }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // Create a real idea, then a node linking to it succeeds.
    let idea: Value = post(&c, format!("{base}/ideas"), json!({ "title": "Real idea" }))
        .await
        .json()
        .await
        .unwrap();
    let idea_id = idea["id"].as_str().unwrap();
    let mm = create_mindmap(
        &c,
        &base,
        json!({
            "title": "Linked",
            "graph": { "nodes": [{ "id": "a", "label": "A", "item_type": "idea", "item_id": idea_id }], "edges": [] }
        }),
    )
    .await;
    assert_eq!(mm["graph"]["nodes"][0]["item_id"], idea_id);
}

#[tokio::test]
async fn from_project_seeds_nodes_and_edges() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let project: Value = post(&c, format!("{base}/projects"), json!({ "name": "Quest" }))
        .await
        .json()
        .await
        .unwrap();
    let pid = project["id"].as_str().unwrap();

    // Two ideas filed under the project (membership via the project items route).
    let mut idea_ids = Vec::new();
    for t in ["Idea one", "Idea two"] {
        let idea: Value = post(&c, format!("{base}/ideas"), json!({ "title": t }))
            .await
            .json()
            .await
            .unwrap();
        let iid = idea["id"].as_str().unwrap().to_string();
        let r = post(
            &c,
            format!("{base}/projects/{pid}/items"),
            json!({ "item_type": "idea", "item_id": iid }),
        )
        .await;
        assert_eq!(r.status(), StatusCode::NO_CONTENT, "add idea to project");
        idea_ids.push(iid);
    }

    let r = post(
        &c,
        format!("{base}/mindmaps/from-project"),
        json!({ "project_id": pid }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let mm: Value = r.json().await.unwrap();
    let nodes = mm["graph"]["nodes"].as_array().unwrap();
    // root + 2 ideas.
    assert_eq!(nodes.len(), 3);
    assert_eq!(mm["project_id"], pid);
    // Membership edges root → idea.
    let edges = mm["graph"]["edges"].as_array().unwrap();
    assert!(edges.len() >= 2);
}

#[tokio::test]
async fn mindmap_appears_in_search_and_scrubs_links_on_delete() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let mm = create_mindmap(
        &c,
        &base,
        json!({ "title": "Zephyr architecture", "outline": "# Zephyr\n- Kernel\n- Scheduler" }),
    )
    .await;
    let mm_id = id_of(&mm);

    // Full-text search finds it via title or node labels.
    let r = c
        .get(format!("{base}/knowledge/search?q=Zephyr"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let results: Value = r.json().await.unwrap();
    let hits = results["mindmaps"].as_array().unwrap();
    assert!(hits.iter().any(|h| h["id"] == mm_id), "mindmap should rank");

    // Deleting removes it (link scrub-on-delete is exercised in the MCP tests,
    // where cross-type links can be created via knowledge_link).
    let r = c
        .delete(format!("{base}/mindmaps/{mm_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn owner_isolation_at_repo_layer() {
    let (_base, state) = spawn().await;
    use baitler_api::mindmap::model::Graph;
    use baitler_api::mindmap::repo;

    let g = Graph::default();
    let a = repo::create_mindmap(
        &state.db,
        "owner-a",
        "A map",
        &g,
        "json",
        None,
        None,
        &[],
        "published",
    )
    .await
    .unwrap();
    // owner-b cannot see owner-a's mindmap.
    assert!(repo::get_mindmap(&state.db, "owner-b", &a.uuid)
        .await
        .unwrap()
        .is_none());
    assert!(repo::get_mindmap(&state.db, "owner-a", &a.uuid)
        .await
        .unwrap()
        .is_some());
    let b_list = repo::list_mindmaps(&state.db, "owner-b", None, None, None, None, None, 100, 0)
        .await
        .unwrap();
    assert!(b_list.is_empty());
}
