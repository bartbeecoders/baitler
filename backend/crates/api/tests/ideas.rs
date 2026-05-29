//! Integration tests for idea management.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{Config, StorageConfig, SurrealConfig};
use baitler_api::{ideas::repo, AppState};
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
                .join("baitler-test-ideas")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
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
        .post(format!("{base}/ideas"))
        .json(&payload)
        .send()
        .await
        .expect("create");
    assert_eq!(resp.status(), 201);
    resp.json().await.unwrap()
}

fn id_of(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn idea_crud_lifecycle() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    let created = create(
        &client,
        &base,
        json!({ "title": "Plan", "body": "# Hi", "tags": ["work"], "status": "active" }),
    )
    .await;
    let id = id_of(&created);
    assert_eq!(created["status"], "active");
    assert_eq!(created["tags"][0], "work");

    // Detail.
    let detail: Value = client
        .get(format!("{base}/ideas/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["title"], "Plan");
    assert_eq!(detail["related"].as_array().unwrap().len(), 0);

    // Patch title/body/tags/status.
    let patched: Value = client
        .patch(format!("{base}/ideas/{id}"))
        .json(&json!({ "title": "Plan v2", "tags": ["work", "q3"], "status": "done" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(patched["title"], "Plan v2");
    assert_eq!(patched["status"], "done");
    assert_eq!(patched["tags"].as_array().unwrap().len(), 2);

    // Delete then 404.
    let del = client
        .delete(format!("{base}/ideas/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);
    let after = client
        .get(format!("{base}/ideas/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), 404);
}

#[tokio::test]
async fn list_filters_by_status_tag_and_search() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    create(
        &client,
        &base,
        json!({ "title": "Alpha report", "tags": ["work"], "status": "active" }),
    )
    .await;
    create(
        &client,
        &base,
        json!({ "title": "Beta notes", "tags": ["personal"], "status": "inbox" }),
    )
    .await;
    create(
        &client,
        &base,
        json!({ "title": "Gamma", "body": "report inside", "status": "active" }),
    )
    .await;

    let count = |v: &Value| v["ideas"].as_array().unwrap().len();

    let active: Value = client
        .get(format!("{base}/ideas?status=active"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(count(&active), 2);

    let work: Value = client
        .get(format!("{base}/ideas?tag=work"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(count(&work), 1);

    // Search matches title OR body, case-insensitive.
    let search: Value = client
        .get(format!("{base}/ideas?q=REPORT"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(count(&search), 2);

    // Distinct tags.
    let tags: Value = client
        .get(format!("{base}/ideas/tags"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tags["tags"], json!(["personal", "work"]));
}

#[tokio::test]
async fn links_are_symmetric_and_scrubbed_on_delete() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    let a = id_of(&create(&client, &base, json!({ "title": "A" })).await);
    let b = id_of(&create(&client, &base, json!({ "title": "B" })).await);

    // Link A → B; both sides see the relation.
    let linked = client
        .post(format!("{base}/ideas/{a}/links"))
        .json(&json!({ "target_id": b }))
        .send()
        .await
        .unwrap();
    assert_eq!(linked.status(), 204);

    let detail_a: Value = client
        .get(format!("{base}/ideas/{a}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let detail_b: Value = client
        .get(format!("{base}/ideas/{b}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail_a["related"][0]["id"], b);
    assert_eq!(detail_b["related"][0]["id"], a);

    // Unlink removes from both.
    let unlinked = client
        .delete(format!("{base}/ideas/{a}/links/{b}"))
        .send()
        .await
        .unwrap();
    assert_eq!(unlinked.status(), 204);
    let detail_a2: Value = client
        .get(format!("{base}/ideas/{a}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail_a2["related"].as_array().unwrap().len(), 0);

    // Relink, then deleting B scrubs the dangling link from A.
    client
        .post(format!("{base}/ideas/{a}/links"))
        .json(&json!({ "target_id": b }))
        .send()
        .await
        .unwrap();
    client
        .delete(format!("{base}/ideas/{b}"))
        .send()
        .await
        .unwrap();
    let detail_a3: Value = client
        .get(format!("{base}/ideas/{a}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail_a3["related"].as_array().unwrap().len(), 0);
    assert_eq!(detail_a3["links"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn validation_rejects_bad_input() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // Empty title.
    let empty = client
        .post(format!("{base}/ideas"))
        .json(&json!({ "title": "  " }))
        .send()
        .await
        .unwrap();
    assert_eq!(empty.status(), 400);

    // Invalid status.
    let bad_status = client
        .post(format!("{base}/ideas"))
        .json(&json!({ "title": "X", "status": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_status.status(), 400);

    // Self-link.
    let a = id_of(&create(&client, &base, json!({ "title": "A" })).await);
    let self_link = client
        .post(format!("{base}/ideas/{a}/links"))
        .json(&json!({ "target_id": a }))
        .send()
        .await
        .unwrap();
    assert_eq!(self_link.status(), 400);
}

#[tokio::test]
async fn owner_scoping_isolates_ideas() {
    let (_base, state) = spawn().await;
    let db = &state.db;

    let alice = repo::create_idea(db, "alice", "A", "", &[], "inbox")
        .await
        .unwrap();
    repo::create_idea(db, "bob", "B", "", &[], "inbox")
        .await
        .unwrap();

    let alice_ideas = repo::list_ideas(db, "alice", None, None, None, 100, 0)
        .await
        .unwrap();
    assert_eq!(alice_ideas.len(), 1);
    assert_eq!(alice_ideas[0].uuid, alice.uuid);

    assert!(repo::get_idea(db, "alice", &alice.uuid)
        .await
        .unwrap()
        .is_some());
    assert!(repo::get_idea(db, "bob", &alice.uuid)
        .await
        .unwrap()
        .is_none());
}
