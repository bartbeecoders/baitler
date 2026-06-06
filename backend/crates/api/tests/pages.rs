//! Integration tests for hosted pages (Phase 12, Milestone A): CRUD, slugs,
//! sanitize-on-write, the visibility/publish state machine, version rules,
//! folder filing/search, the `from_document` promote, and project membership +
//! cross-type link scrub-on-delete. All on an embedded `memory` DB, no egress.

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
                .join("baitler-test-pages")
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
async fn create_page(c: &Client, base: &str, body: Value) -> Value {
    let r = post(c, format!("{base}/pages"), body).await;
    assert_eq!(r.status(), StatusCode::CREATED, "create page");
    r.json().await.unwrap()
}
fn id_of(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn page_crud_lifecycle() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let page = create_page(
        &c,
        &base,
        json!({ "title": "Hello World", "body": "<p>hi</p>" }),
    )
    .await;
    let id = id_of(&page);
    assert_eq!(page["slug"], "hello-world");
    assert_eq!(page["visibility"], "draft");
    assert_eq!(page["source_format"], "html");
    assert_eq!(page["version"], 1);
    assert_eq!(page["public_url"], "", "a draft has no public url");

    // Get.
    let got: Value = c
        .get(format!("{base}/pages/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got["title"], "Hello World");

    // List (summary omits body).
    let list: Value = c
        .get(format!("{base}/pages"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list["pages"].as_array().unwrap().len(), 1);
    assert!(list["pages"][0].get("body").is_none(), "summary omits body");

    // Patch title.
    let patched: Value = c
        .patch(format!("{base}/pages/{id}"))
        .json(&json!({ "title": "Renamed" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(patched["title"], "Renamed");

    // Delete then 404.
    assert_eq!(
        c.delete(format!("{base}/pages/{id}"))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        c.get(format!("{base}/pages/{id}"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn markdown_is_rendered_and_html_is_sanitized_on_write() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    // Markdown source → rendered to HTML, then sanitized.
    let md = create_page(
        &c,
        &base,
        json!({ "title": "MD", "body": "# Title\n\nhello", "source_format": "markdown" }),
    )
    .await;
    assert!(md["body"].as_str().unwrap().contains("<h1>"));

    // A <script>/onerror in HTML never reaches the stored body.
    let dirty = create_page(&c, &base, json!({ "title": "X", "body": "<p>ok</p><script>alert(1)</script><img src=x onerror=alert(1)>" })).await;
    let body = dirty["body"].as_str().unwrap();
    assert!(!body.contains("<script"), "script stripped");
    assert!(!body.contains("onerror"), "handler stripped");
    assert!(body.contains("<p>ok</p>"));
}

#[tokio::test]
async fn slug_collisions_and_custom_slug() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let a = create_page(&c, &base, json!({ "title": "Same Name" })).await;
    let b = create_page(&c, &base, json!({ "title": "Same Name" })).await;
    assert_eq!(a["slug"], "same-name");
    assert_eq!(b["slug"], "same-name-2", "collision gets a numeric suffix");

    // A custom slug is normalized to the slug charset.
    let id = id_of(&b);
    let renamed: Value = c
        .patch(format!("{base}/pages/{id}"))
        .json(&json!({ "slug": "My Custom Slug!" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(renamed["slug"], "my-custom-slug");

    // Re-using another page's slug is a 409.
    let conflict = c
        .patch(format!("{base}/pages/{id}"))
        .json(&json!({ "slug": "same-name" }))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), 409);
}

#[tokio::test]
async fn visibility_publish_unpublish_and_public_url() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    let id = id_of(&create_page(&c, &base, json!({ "title": "Landing" })).await);

    // Publish → public, public_url + published_at set.
    let pub_: Value = post(&c, format!("{base}/pages/{id}/publish"), json!({}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(pub_["visibility"], "public");
    assert_eq!(pub_["public_url"], "/p/landing");
    assert!(pub_["published_at"].is_string(), "published_at stamped");

    // Unpublish → draft, public_url cleared.
    let unp: Value = post(&c, format!("{base}/pages/{id}/unpublish"), json!({}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(unp["visibility"], "draft");
    assert_eq!(unp["public_url"], "");
    assert!(unp["published_at"].is_null(), "published_at cleared");

    // PATCH visibility=unlisted works inline too.
    let un: Value = c
        .patch(format!("{base}/pages/{id}"))
        .json(&json!({ "visibility": "unlisted" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(un["visibility"], "unlisted");
    assert!(un["public_url"].as_str().unwrap().starts_with("/p/"));

    // Bad visibility rejected.
    let bad = c
        .patch(format!("{base}/pages/{id}"))
        .json(&json!({ "visibility": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);
}

#[tokio::test]
async fn version_bumps_on_content_edit_not_visibility() {
    let (base, _s) = spawn().await;
    let c = Client::new();
    let id = id_of(&create_page(&c, &base, json!({ "title": "V", "body": "<p>1</p>" })).await);

    // Content edit bumps version.
    let v2: Value = c
        .patch(format!("{base}/pages/{id}"))
        .json(&json!({ "body": "<p>2</p>" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v2["version"], 2);

    // Publish (visibility-only) does NOT bump version.
    let pubd: Value = post(
        &c,
        format!("{base}/pages/{id}/publish"),
        json!({ "visibility": "unlisted" }),
    )
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(pubd["version"], 2, "visibility flip is not a content edit");
}

#[tokio::test]
async fn folders_search_and_folder_not_empty_with_page() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let folder: Value = post(&c, format!("{base}/folders"), json!({ "name": "site" }))
        .await
        .json()
        .await
        .unwrap();
    let fid = folder["id"].as_str().unwrap().to_string();

    create_page(&c, &base, json!({ "title": "Quarterly Ownership", "body": "<p>borrow checker notes</p>", "folder_id": fid })).await;
    create_page(&c, &base, json!({ "title": "Cooking" })).await;

    // Filter by folder.
    let in_folder: Value = c
        .get(format!("{base}/pages?folder={fid}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(in_folder["pages"].as_array().unwrap().len(), 1);

    // Search q (case-insensitive title/body).
    let found: Value = c
        .get(format!("{base}/pages?q=BORROW"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(found["pages"].as_array().unwrap().len(), 1);

    // A folder holding a page is not empty → delete 409.
    let del = c
        .delete(format!("{base}/folders/{fid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 409, "folder with a page can't be deleted");
}

#[tokio::test]
async fn from_document_promote_copies_body() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    let doc: Value = post(
        &c,
        format!("{base}/documents"),
        json!({ "title": "Spec", "body": "<h2>spec body</h2>" }),
    )
    .await
    .json()
    .await
    .unwrap();
    let did = doc["id"].as_str().unwrap().to_string();

    let page = create_page(
        &c,
        &base,
        json!({ "title": "Spec Page", "from_document": did }),
    )
    .await;
    assert!(page["body"].as_str().unwrap().contains("spec body"));
    assert_eq!(page["source_format"], "html");

    // A bogus from_document is a 400.
    let bad = post(
        &c,
        format!("{base}/pages"),
        json!({ "title": "X", "from_document": "ghost" }),
    )
    .await;
    assert_eq!(bad.status(), 400);
}

#[tokio::test]
async fn pages_join_projects_and_links_scrub_on_delete() {
    let (base, state) = spawn().await;
    let c = Client::new();
    use baitler_api::ideas::repo as ideas_repo;
    use baitler_api::knowledge::repo as kn;
    let db = &state.db;

    let project: Value = post(&c, format!("{base}/projects"), json!({ "name": "Site" }))
        .await
        .json()
        .await
        .unwrap();
    let pid = project["id"].as_str().unwrap().to_string();
    let page = create_page(&c, &base, json!({ "title": "Home", "project_id": pid })).await;
    let page_id = id_of(&page);

    // The page counts as a project member.
    let detail: Value = c
        .get(format!("{base}/projects/{pid}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["counts"]["pages"], 1);
    assert_eq!(detail["members"]["pages"][0]["id"], page_id);

    // Cross-type link (repo-level, since there is no link REST endpoint) is
    // scrubbed when the page is deleted.
    let idea = ideas_repo::create_idea(db, "dev", "Note", "", &[], "inbox", "published", None)
        .await
        .unwrap();
    kn::link_items(db, "dev", "page", &page_id, "idea", &idea.uuid, "documents")
        .await
        .unwrap();
    assert_eq!(
        kn::backlinks(db, "dev", "idea", &idea.uuid)
            .await
            .unwrap()
            .len(),
        1
    );

    assert_eq!(
        c.delete(format!("{base}/pages/{page_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert!(
        kn::backlinks(db, "dev", "idea", &idea.uuid)
            .await
            .unwrap()
            .is_empty(),
        "link scrubbed on page delete"
    );
}

#[tokio::test]
async fn pages_are_owner_scoped() {
    let (_base, state) = spawn().await;
    use baitler_api::pages::repo as pages;
    let db = &state.db;

    let alice = pages::create_page(
        db,
        "alice",
        "A",
        "<p>a</p>",
        "html",
        "draft",
        None,
        None,
        &[],
    )
    .await
    .unwrap();
    pages::create_page(db, "bob", "B", "<p>b</p>", "html", "draft", None, None, &[])
        .await
        .unwrap();

    assert_eq!(
        pages::list_pages(db, "alice", None, None, None, None, None, 100, 0)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(pages::get_page(db, "bob", &alice.uuid)
        .await
        .unwrap()
        .is_none());
}

/// The whole point of the phase: a published page is served at GET /p/{slug} as
/// locked-down HTML from outside the credentialed CORS layer, drafts 404, and a
/// remote <img>/<script> never reaches the served bytes.
#[tokio::test]
async fn public_serving_and_security_headers() {
    let (base, _s) = spawn().await;
    let c = Client::new();

    const EXPECTED_CSP: &str = "default-src 'none'; img-src data:; style-src 'unsafe-inline'; \
         base-uri 'none'; form-action 'none'; frame-ancestors 'none'; sandbox";

    // A public page whose body embeds a <script> and an internal-host remote <img>.
    let pubp = create_page(
        &c,
        &base,
        json!({
            "title": "Public Page",
            "body": "<h1>Hello</h1><script>alert(1)</script><img src=\"http://169.254.169.254/x\">"
        }),
    )
    .await;
    let pub_id = id_of(&pubp);
    post(
        &c,
        format!("{base}/pages/{pub_id}/publish"),
        json!({ "visibility": "public" }),
    )
    .await;
    let pub_slug = pubp["slug"].as_str().unwrap();

    let unlisted = create_page(
        &c,
        &base,
        json!({ "title": "Secret", "visibility": "unlisted", "body": "<p>shh</p>" }),
    )
    .await;
    let un_slug = unlisted["slug"].as_str().unwrap().to_string();

    let draft = create_page(&c, &base, json!({ "title": "Draft" })).await;
    let draft_slug = draft["slug"].as_str().unwrap().to_string();

    // ── public: 200, exact locked-down headers, sanitized + hardened body ──
    let r = c
        .get(format!("{base}/p/{pub_slug}"))
        .header("Origin", "http://localhost:8100")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let h = r.headers().clone();
    assert_eq!(
        h.get("content-security-policy").unwrap(),
        EXPECTED_CSP,
        "exact CSP"
    );
    assert_eq!(h.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(h.get("referrer-policy").unwrap(), "no-referrer");
    assert!(h
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    // public pages are indexable: no noindex / no-store.
    assert!(h.get("x-robots-tag").is_none());
    assert!(h.get("cache-control").is_none());
    // CRUCIAL: the public route is outside the credentialed CORS layer.
    assert!(
        h.get("access-control-allow-credentials").is_none(),
        "/p/* must not be credentialed-CORS"
    );
    let body = r.text().await.unwrap();
    assert!(body.contains("<h1>Hello</h1>"));
    assert!(!body.contains("<script"), "script never served");
    assert!(
        !body.contains("169.254.169.254"),
        "remote img stripped by harden_for_render at serve"
    );

    // ── unlisted: 200 + noindex + no-store ──
    let ru = c.get(format!("{base}/p/{un_slug}")).send().await.unwrap();
    assert_eq!(ru.status(), StatusCode::OK);
    assert_eq!(ru.headers().get("x-robots-tag").unwrap(), "noindex");
    assert_eq!(
        ru.headers().get("cache-control").unwrap(),
        "private, no-store"
    );

    // ── draft / unknown: 404 (existence never confirmed) ──
    assert_eq!(
        c.get(format!("{base}/p/{draft_slug}"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{base}/p/does-not-exist"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}
