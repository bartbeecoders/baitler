//! Integration tests for the Phase 11 knowledge REST API (the portal surface):
//! projects, membership, the review queue, search, and the activity endpoint.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{Config, McpConfig, StorageConfig, SurrealConfig};
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
                .join("baitler-test-projects")
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

async fn spawn() -> String {
    let state: AppState = baitler_api::build_state(test_config())
        .await
        .expect("state");
    let app = baitler_api::build_app(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    format!("http://{addr}")
}

async fn post(client: &Client, url: String, body: Value) -> reqwest::Response {
    client.post(url).json(&body).send().await.expect("post")
}

#[tokio::test]
async fn project_lifecycle_membership_review_and_search() {
    let base = spawn().await;
    let client = Client::new();

    // Create a project.
    let resp = post(
        &client,
        format!("{base}/projects"),
        json!({ "name": "Quarterly Report", "summary": "Q3 numbers" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project: Value = resp.json().await.unwrap();
    assert_eq!(project["slug"], "quarterly-report");
    let pid = project["id"].as_str().unwrap().to_string();

    // Create an idea and a document, file them under the project.
    let idea: Value = post(
        &client,
        format!("{base}/ideas"),
        json!({ "title": "Revenue note", "body": "growth was strong" }),
    )
    .await
    .json()
    .await
    .unwrap();
    let iid = idea["id"].as_str().unwrap().to_string();
    let doc: Value = post(
        &client,
        format!("{base}/documents"),
        json!({ "title": "Summary", "body": "<p>strong quarter</p>" }),
    )
    .await
    .json()
    .await
    .unwrap();
    let did = doc["id"].as_str().unwrap().to_string();

    for (ty, id) in [("idea", &iid), ("document", &did)] {
        let r = post(
            &client,
            format!("{base}/projects/{pid}/items"),
            json!({ "item_type": ty, "item_id": id }),
        )
        .await;
        assert_eq!(r.status(), StatusCode::NO_CONTENT);
    }

    // Project detail reflects membership.
    let detail: Value = client
        .get(format!("{base}/projects/{pid}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["counts"]["ideas"], 1);
    assert_eq!(detail["counts"]["documents"], 1);
    assert_eq!(detail["name"], "Quarterly Report");
    assert_eq!(detail["members"]["ideas"][0]["id"], iid);

    // Review queue: REST writes are published, so the queue starts empty…
    let queue: Value = client
        .get(format!("{base}/review"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(queue["ideas"].as_array().unwrap().len(), 0);
    // …flip the idea to draft → it appears; approve (publish) → it's gone.
    let to_draft = client
        .patch(format!("{base}/ideas/{iid}"))
        .json(&json!({ "review": "draft" }))
        .send()
        .await
        .unwrap();
    assert_eq!(to_draft.status(), 200);
    let queue: Value = client
        .get(format!("{base}/review"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(queue["ideas"].as_array().unwrap().len(), 1);
    assert_eq!(queue["ideas"][0]["id"], iid);
    let approve: Value = client
        .patch(format!("{base}/ideas/{iid}"))
        .json(&json!({ "review": "published" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(approve["review"], "published");
    let queue: Value = client
        .get(format!("{base}/review"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(queue["ideas"].as_array().unwrap().len(), 0);

    // Search finds the document under the project's text.
    let search: Value = client
        .get(format!("{base}/knowledge/search?q=quarter"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!search["documents"].as_array().unwrap().is_empty());

    // Invalid review is rejected.
    let bad = client
        .patch(format!("{base}/ideas/{iid}"))
        .json(&json!({ "review": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);

    // Activity endpoint responds (REST writes aren't logged yet → array present).
    let activity: Value = client
        .get(format!("{base}/activity"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(activity["activity"].is_array());

    // Remove a member, then delete the project.
    let rm = client
        .delete(format!("{base}/projects/{pid}/items/document/{did}"))
        .send()
        .await
        .unwrap();
    assert_eq!(rm.status(), StatusCode::NO_CONTENT);
    let detail: Value = client
        .get(format!("{base}/projects/{pid}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["counts"]["documents"], 0);

    let del = client
        .delete(format!("{base}/projects/{pid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);
    let after = client
        .get(format!("{base}/projects/{pid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), 404);
    // The member items themselves survive the project delete.
    assert_eq!(
        client
            .get(format!("{base}/ideas/{iid}"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}
