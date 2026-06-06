//! Integration tests for file storage & management.
//!
//! Each test boots a full app instance (embedded memory DB + local storage in a
//! temp dir) and drives it over HTTP. Owner-scoping is exercised at the repo
//! layer with two synthetic owners, since the HTTP layer's owner is a fixed stub
//! until auth lands.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{CliConfig, Config, McpConfig, StorageConfig, SurrealConfig};
use baitler_api::{files::repo, AppState};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde_json::Value;
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
                .join("baitler-test-files")
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
        public_page_origin: None,
        secret_key: [7u8; 32],
    }
}

/// Boot an app instance; return its base URL and a handle to shared state.
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

async fn upload(
    client: &Client,
    base: &str,
    folder: Option<&str>,
    name: &str,
    body: &[u8],
) -> Value {
    let url = match folder {
        Some(f) => format!("{base}/files?folder={f}"),
        None => format!("{base}/files"),
    };
    let part = Part::bytes(body.to_vec())
        .file_name(name.to_string())
        .mime_str("text/plain")
        .unwrap();
    let form = Form::new().part("file", part);
    let resp = client
        .post(url)
        .multipart(form)
        .send()
        .await
        .expect("upload");
    assert_eq!(resp.status(), 201, "upload should return 201");
    resp.json().await.expect("json")
}

async fn upload_with_mime(
    client: &Client,
    base: &str,
    name: &str,
    mime: &str,
    body: &[u8],
) -> String {
    let part = Part::bytes(body.to_vec())
        .file_name(name.to_string())
        .mime_str(mime)
        .unwrap();
    let resp = client
        .post(format!("{base}/files"))
        .multipart(Form::new().part("file", part))
        .send()
        .await
        .expect("upload");
    assert_eq!(resp.status(), 201);
    resp.json::<Value>().await.unwrap()["files"][0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_folder(client: &Client, base: &str, name: &str, parent: Option<&str>) -> String {
    let mut body = serde_json::json!({ "name": name });
    if let Some(p) = parent {
        body["parent_id"] = serde_json::json!(p);
    }
    let resp = client
        .post(format!("{base}/folders"))
        .json(&body)
        .send()
        .await
        .expect("create folder");
    assert_eq!(resp.status(), 201);
    resp.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn file_lifecycle_upload_list_download_rename_delete() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // Upload
    let created = upload(&client, &base, None, "hello.txt", b"hello world").await;
    let id = created["files"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(created["files"][0]["name"], "hello.txt");
    assert_eq!(created["files"][0]["size"], 11);

    // List root shows the file
    let listing: Value = client
        .get(format!("{base}/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = listing["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"hello.txt"));

    // Download returns the bytes and the stored mime
    let dl = client
        .get(format!("{base}/files/{id}/content"))
        .send()
        .await
        .unwrap();
    assert_eq!(dl.status(), 200);
    assert!(dl
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    assert_eq!(dl.bytes().await.unwrap().as_ref(), b"hello world");

    // Rename
    let renamed = client
        .patch(format!("{base}/files/{id}"))
        .json(&serde_json::json!({ "name": "renamed.txt" }))
        .send()
        .await
        .unwrap();
    assert_eq!(renamed.status(), 200);
    assert_eq!(
        renamed.json::<Value>().await.unwrap()["name"],
        "renamed.txt"
    );

    // Delete, then 404
    let del = client
        .delete(format!("{base}/files/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);
    let after = client
        .get(format!("{base}/files/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), 404);
    let body: Value = after.json().await.unwrap();
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn folders_move_and_nonempty_delete() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // Create a folder
    let folder: Value = client
        .post(format!("{base}/folders"))
        .json(&serde_json::json!({ "name": "Docs" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let folder_id = folder["id"].as_str().unwrap().to_string();

    // Upload into the folder
    let created = upload(&client, &base, Some(&folder_id), "in-folder.txt", b"abc").await;
    let file_id = created["files"][0]["id"].as_str().unwrap().to_string();

    // Listing the folder shows the file + a breadcrumb
    let listing: Value = client
        .get(format!("{base}/files?folder={folder_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listing["folder"]["id"], folder_id);
    assert_eq!(listing["breadcrumbs"].as_array().unwrap().len(), 1);
    assert_eq!(listing["files"].as_array().unwrap().len(), 1);

    // Deleting a non-empty folder is a 409
    let del = client
        .delete(format!("{base}/folders/{folder_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 409);
    assert_eq!(
        del.json::<Value>().await.unwrap()["error"]["code"],
        "conflict"
    );

    // Move the file to root (folder_id: null), then the folder is deletable
    let moved = client
        .patch(format!("{base}/files/{file_id}"))
        .json(&serde_json::json!({ "folder_id": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(moved.status(), 200);
    assert!(moved.json::<Value>().await.unwrap()["folder_id"].is_null());

    let del2 = client
        .delete(format!("{base}/folders/{folder_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del2.status(), 204);
}

#[tokio::test]
async fn search_matches_by_name() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    upload(&client, &base, None, "budget-2026.txt", b"x").await;
    upload(&client, &base, None, "notes.txt", b"y").await;

    let results: Value = client
        .get(format!("{base}/files?q=budget"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let files = results["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["name"], "budget-2026.txt");
}

#[tokio::test]
async fn owner_scoping_isolates_files() {
    let (_base, state) = spawn().await;
    let db = &state.db;

    // Two owners create files directly via the repo layer.
    let alice = repo::create_file(
        db,
        "alice",
        "a-uuid",
        "a.txt",
        "text/plain",
        1,
        None,
        "a-key",
    )
    .await
    .unwrap();
    let _bob = repo::create_file(db, "bob", "b-uuid", "b.txt", "text/plain", 1, None, "b-key")
        .await
        .unwrap();

    // Alice sees only her file...
    let alice_files = repo::list_files(db, "alice", None).await.unwrap();
    assert_eq!(alice_files.len(), 1);
    assert_eq!(alice_files[0].uuid, alice.uuid);

    // ...and cannot fetch Bob's by id.
    assert!(repo::get_file(db, "alice", "b-uuid")
        .await
        .unwrap()
        .is_none());
    assert!(repo::get_file(db, "bob", "b-uuid").await.unwrap().is_some());
}

#[tokio::test]
async fn move_file_into_folder_then_rename_only() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    let created = upload(&client, &base, None, "doc.txt", b"x").await;
    let file_id = created["files"][0]["id"].as_str().unwrap().to_string();
    let folder_id = create_folder(&client, &base, "Target", None).await;

    // Move the file into the folder.
    let moved = client
        .patch(format!("{base}/files/{file_id}"))
        .json(&serde_json::json!({ "folder_id": folder_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(moved.status(), 200);
    assert_eq!(moved.json::<Value>().await.unwrap()["folder_id"], folder_id);

    // It now appears in the folder and not at the root.
    let in_folder: Value = client
        .get(format!("{base}/files?folder={folder_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(in_folder["files"].as_array().unwrap().len(), 1);
    let root: Value = client
        .get(format!("{base}/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(root["files"].as_array().unwrap().len(), 0);

    // A name-only PATCH must NOT clear folder_id (guards the double-option model).
    let renamed = client
        .patch(format!("{base}/files/{file_id}"))
        .json(&serde_json::json!({ "name": "renamed.txt" }))
        .send()
        .await
        .unwrap();
    let body: Value = renamed.json().await.unwrap();
    assert_eq!(body["name"], "renamed.txt");
    assert_eq!(body["folder_id"], folder_id);

    // Moving into a non-existent folder is a 400.
    let bad = client
        .patch(format!("{base}/files/{file_id}"))
        .json(&serde_json::json!({ "folder_id": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);
}

#[tokio::test]
async fn folder_move_cycle_guards() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    let a = create_folder(&client, &base, "A", None).await;
    let b = create_folder(&client, &base, "B", Some(&a)).await;
    let c = create_folder(&client, &base, "C", Some(&b)).await;
    let d = create_folder(&client, &base, "D", None).await;

    // Move into self → 400.
    let into_self = client
        .patch(format!("{base}/folders/{a}"))
        .json(&serde_json::json!({ "parent_id": a }))
        .send()
        .await
        .unwrap();
    assert_eq!(into_self.status(), 400);

    // Move A into its descendant C → 400.
    let into_desc = client
        .patch(format!("{base}/folders/{a}"))
        .json(&serde_json::json!({ "parent_id": c }))
        .send()
        .await
        .unwrap();
    assert_eq!(into_desc.status(), 400);

    // Valid move: C under sibling D → 200, breadcrumbs reflect [D, C].
    let ok = client
        .patch(format!("{base}/folders/{c}"))
        .json(&serde_json::json!({ "parent_id": d }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    let listing: Value = client
        .get(format!("{base}/files?folder={c}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let crumbs = listing["breadcrumbs"].as_array().unwrap();
    assert_eq!(crumbs.len(), 2);
    assert_eq!(crumbs[0]["id"], d);
    assert_eq!(crumbs[1]["id"], c);
}

#[tokio::test]
async fn upload_and_listing_validation_errors() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // Upload into a non-existent folder → 400.
    let part = Part::bytes(b"x".to_vec())
        .file_name("x.txt")
        .mime_str("text/plain")
        .unwrap();
    let bad_folder = client
        .post(format!("{base}/files?folder=does-not-exist"))
        .multipart(Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_folder.status(), 400);

    // Multipart with no "file" field → 400.
    let other = Part::bytes(b"x".to_vec())
        .file_name("x.txt")
        .mime_str("text/plain")
        .unwrap();
    let no_file = client
        .post(format!("{base}/files"))
        .multipart(Form::new().part("notfile", other))
        .send()
        .await
        .unwrap();
    assert_eq!(no_file.status(), 400);

    // Listing a non-existent folder → 404.
    let nope = client
        .get(format!("{base}/files?folder=nope"))
        .send()
        .await
        .unwrap();
    assert_eq!(nope.status(), 404);
}

#[tokio::test]
async fn oversized_upload_is_rejected() {
    let (base, _state) = spawn().await; // test_config caps uploads at 16 MiB
    let client = Client::new();

    let big = vec![0u8; 17 * 1024 * 1024];
    let part = Part::bytes(big)
        .file_name("big.bin")
        .mime_str("application/octet-stream")
        .unwrap();
    let resp = client
        .post(format!("{base}/files"))
        .multipart(Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 413, "over-limit upload should be 413");
}

#[tokio::test]
async fn search_is_case_insensitive_cross_folder_and_paginated() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    let folder = create_folder(&client, &base, "Sub", None).await;
    upload(&client, &base, None, "Report-A.txt", b"x").await;
    upload(&client, &base, None, "report-b.txt", b"x").await;
    upload(&client, &base, Some(&folder), "REPORT-c.txt", b"x").await;

    // Case-insensitive + spans folders.
    let all: Value = client
        .get(format!("{base}/files?q=report"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all["files"].as_array().unwrap().len(), 3);

    // Pagination: limit + offset cover distinct results with no overlap.
    let page1: Value = client
        .get(format!("{base}/files?q=report&limit=2"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page2: Value = client
        .get(format!("{base}/files?q=report&limit=2&offset=2"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page1["files"].as_array().unwrap().len(), 2);
    assert_eq!(page2["files"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn breadcrumbs_order_download_headers_and_cleanup() {
    let (base, state) = spawn().await;
    let client = Client::new();

    // Breadcrumb order A > B > C.
    let a = create_folder(&client, &base, "A", None).await;
    let b = create_folder(&client, &base, "B", Some(&a)).await;
    let c = create_folder(&client, &base, "C", Some(&b)).await;
    let listing: Value = client
        .get(format!("{base}/files?folder={c}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let crumbs = listing["breadcrumbs"].as_array().unwrap();
    assert_eq!(crumbs.len(), 3);
    assert_eq!(crumbs[0]["id"], a);
    assert_eq!(crumbs[2]["id"], c);

    // Download headers: nosniff, content-length, content-disposition filename.
    let created = upload(&client, &base, None, "report.txt", b"hello").await;
    let id = created["files"][0]["id"].as_str().unwrap().to_string();
    let dl = client
        .get(format!("{base}/files/{id}/content"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        dl.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(dl.headers().get("content-length").unwrap(), "5");
    assert!(dl
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("report.txt"));

    // Storage object is removed on delete (public id == storage key).
    assert!(state.storage.open(&id).await.is_ok());
    let del = client
        .delete(format!("{base}/files/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);
    assert!(
        state.storage.open(&id).await.is_err(),
        "storage object should be gone after delete"
    );
}

#[tokio::test]
async fn invalid_names_are_rejected() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    for bad in ["", "  ", "a/b"] {
        let resp = client
            .post(format!("{base}/folders"))
            .json(&serde_json::json!({ "name": bad }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400, "name {bad:?} should be rejected");
        assert_eq!(
            resp.json::<Value>().await.unwrap()["error"]["code"],
            "bad_request"
        );
    }
}

#[tokio::test]
async fn download_disposition_enforces_inline_allowlist() {
    let (base, _state) = spawn().await;
    let client = Client::new();

    // PNG is on the inline allowlist.
    let png = upload_with_mime(&client, &base, "img.png", "image/png", b"\x89PNG\r\n").await;
    let png_dl = client
        .get(format!("{base}/files/{png}/content"))
        .send()
        .await
        .unwrap();
    assert_eq!(png_dl.headers().get("content-type").unwrap(), "image/png");
    assert!(png_dl
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("inline"));

    // SVG can carry script, so it must be served as an attachment, never inline.
    let svg = upload_with_mime(&client, &base, "x.svg", "image/svg+xml", b"<svg/>").await;
    let svg_dl = client
        .get(format!("{base}/files/{svg}/content"))
        .send()
        .await
        .unwrap();
    assert!(svg_dl
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("attachment"));
    assert_eq!(
        svg_dl.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );

    // A non-ASCII filename still yields a valid Content-Disposition (RFC 6266 filename*).
    let uni = upload_with_mime(&client, &base, "résumé.txt", "text/plain", b"x").await;
    let uni_dl = client
        .get(format!("{base}/files/{uni}/content"))
        .send()
        .await
        .unwrap();
    assert!(uni_dl
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("filename*=UTF-8''"));
}
