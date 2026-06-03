//! Repo-level integration tests for the Phase 11 knowledge layer: projects,
//! membership, cross-type links, backlinks, and scrub-on-delete. Each test runs
//! against an embedded `memory` DB with migrations applied.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use baitler_api::config::{CliConfig, Config, McpConfig, StorageConfig, SurrealConfig};
use baitler_api::documents::repo as doc_repo;
use baitler_api::files::repo as files_repo;
use baitler_api::ideas::repo as ideas_repo;
use baitler_api::knowledge::repo as kn;
use baitler_api::AppState;

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
                .join("baitler-test-knowledge")
                .to_string_lossy()
                .into_owned(),
            max_upload_bytes: 16 * 1024 * 1024,
        },
        mcp: McpConfig {
            enabled: true,
            auth_token: None,
        },
        cli: CliConfig::default(),
        public_page_origin: None,
        secret_key: [7u8; 32],
    }
}

async fn state() -> AppState {
    baitler_api::build_state(test_config())
        .await
        .expect("build state")
}

#[tokio::test]
async fn project_crud_and_unique_slug() {
    let s = state().await;
    let db = &s.db;

    let p1 = kn::create_project(db, "dev", "My Cool Project", "about")
        .await
        .unwrap();
    assert_eq!(p1.slug, "my-cool-project");
    assert_eq!(p1.status, "active");

    // Same name → a distinct slug, not a unique-index error.
    let p2 = kn::create_project(db, "dev", "My Cool Project", "")
        .await
        .unwrap();
    assert_eq!(p2.slug, "my-cool-project-2");

    let listed = kn::list_projects(db, "dev").await.unwrap();
    assert_eq!(listed.len(), 2);

    let updated = kn::update_project(db, "dev", &p1.uuid, Some("Renamed"), None, Some("archived"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.status, "archived");
    // Slug is stable across a rename (it's an identifier, not a title).
    assert_eq!(updated.slug, "my-cool-project");
}

#[tokio::test]
async fn membership_counts_and_detach_on_project_delete() {
    let s = state().await;
    let db = &s.db;

    let proj = kn::create_project(db, "dev", "Docs", "").await.unwrap();
    let idea = ideas_repo::create_idea(db, "dev", "Note", "", &[], "inbox", "draft", None)
        .await
        .unwrap();
    let doc = doc_repo::create_document(db, "dev", "Doc", "<p>x</p>", "published", None, &[])
        .await
        .unwrap();
    let file = files_repo::create_file(db, "dev", "f1", "a.txt", "text/plain", 3, None, "key1")
        .await
        .unwrap();

    kn::set_membership(db, "dev", "idea", &idea.uuid, Some(&proj.uuid))
        .await
        .unwrap();
    kn::set_membership(db, "dev", "document", &doc.uuid, Some(&proj.uuid))
        .await
        .unwrap();
    kn::set_membership(db, "dev", "file", &file.uuid, Some(&proj.uuid))
        .await
        .unwrap();

    let members = kn::project_members(db, "dev", &proj.uuid).await.unwrap();
    assert_eq!(members.ideas.len(), 1);
    assert_eq!(members.documents.len(), 1);
    assert_eq!(members.files.len(), 1);

    let counts = kn::member_counts(db, "dev", &proj.uuid).await.unwrap();
    assert_eq!(counts.ideas, 1);
    assert_eq!(counts.documents, 1);
    assert_eq!(counts.files, 1);
    assert_eq!(counts.drafts, 1, "the draft idea should count as pending");

    // Membership to a non-existent project is rejected.
    assert!(
        kn::set_membership(db, "dev", "idea", &idea.uuid, Some("nope"))
            .await
            .is_err()
    );

    // Deleting the project detaches members but never deletes them.
    assert!(kn::delete_project(db, "dev", &proj.uuid).await.unwrap());
    assert!(ideas_repo::get_idea(db, "dev", &idea.uuid)
        .await
        .unwrap()
        .is_some());
    let after = kn::member_counts(db, "dev", &proj.uuid).await.unwrap();
    assert_eq!(after.ideas, 0, "members detached after project delete");
}

#[tokio::test]
async fn cross_type_links_are_symmetric_and_scrubbed_on_delete() {
    let s = state().await;
    let db = &s.db;

    let idea = ideas_repo::create_idea(db, "dev", "Idea", "", &[], "inbox", "draft", None)
        .await
        .unwrap();
    let doc = doc_repo::create_document(db, "dev", "Doc", "<p>x</p>", "draft", None, &[])
        .await
        .unwrap();

    kn::link_items(
        db,
        "dev",
        "idea",
        &idea.uuid,
        "document",
        &doc.uuid,
        "references",
    )
    .await
    .unwrap();

    // Resolvable from BOTH endpoints (symmetric double-rows).
    let from_idea = kn::backlinks(db, "dev", "idea", &idea.uuid).await.unwrap();
    assert_eq!(from_idea.len(), 1);
    assert_eq!(from_idea[0].item_type, "document");
    assert_eq!(from_idea[0].id, doc.uuid);
    assert_eq!(from_idea[0].title.as_deref(), Some("Doc"));
    assert_eq!(from_idea[0].relation, "references");

    let from_doc = kn::backlinks(db, "dev", "document", &doc.uuid)
        .await
        .unwrap();
    assert_eq!(from_doc.len(), 1);
    assert_eq!(from_doc[0].id, idea.uuid);

    // Idempotent + relation-updating.
    kn::link_items(
        db,
        "dev",
        "idea",
        &idea.uuid,
        "document",
        &doc.uuid,
        "implements",
    )
    .await
    .unwrap();
    let relinked = kn::backlinks(db, "dev", "idea", &idea.uuid).await.unwrap();
    assert_eq!(relinked.len(), 1, "no duplicate edge");
    assert_eq!(relinked[0].relation, "implements");

    // Self-link rejected; link to missing target rejected.
    assert!(
        kn::link_items(db, "dev", "idea", &idea.uuid, "idea", &idea.uuid, "")
            .await
            .is_err()
    );
    assert!(
        kn::link_items(db, "dev", "idea", &idea.uuid, "document", "ghost", "")
            .await
            .is_err()
    );

    // Deleting the document scrubs the link from both directions.
    assert!(doc_repo::delete_document(db, "dev", &doc.uuid)
        .await
        .unwrap());
    let after = kn::backlinks(db, "dev", "idea", &idea.uuid).await.unwrap();
    assert!(
        after.is_empty(),
        "link scrubbed when the target was deleted"
    );
}

#[tokio::test]
async fn cross_type_full_text_search() {
    let s = state().await;
    let db = &s.db;

    ideas_repo::create_idea(
        db,
        "dev",
        "Rust ownership",
        "the borrow checker",
        &[],
        "inbox",
        "published",
        None,
    )
    .await
    .unwrap();
    ideas_repo::create_idea(
        db,
        "dev",
        "Cooking",
        "a bread recipe",
        &[],
        "inbox",
        "published",
        None,
    )
    .await
    .unwrap();
    doc_repo::create_document(
        db,
        "dev",
        "Borrow guide",
        "<p>ownership and lifetimes</p>",
        "published",
        None,
        &[],
    )
    .await
    .unwrap();
    kn::create_project(db, "dev", "Ownership notes", "borrow semantics")
        .await
        .unwrap();

    let results = kn::search(db, "dev", "borrow", 50).await.unwrap();
    // The matching idea + document + project surface in their typed sections.
    assert_eq!(
        results.ideas.len(),
        1,
        "only the Rust idea matches 'borrow'"
    );
    assert_eq!(results.ideas[0].title, "Rust ownership");
    assert_eq!(results.documents.len(), 1);
    assert_eq!(results.projects.len(), 1);
    assert!(results.files.is_empty());

    // A non-matching term returns nothing.
    let none = kn::search(db, "dev", "zzzznomatch", 50).await.unwrap();
    assert!(none.ideas.is_empty() && none.documents.is_empty());

    // Search is owner-scoped.
    let other = kn::search(db, "bob", "borrow", 50).await.unwrap();
    assert!(other.ideas.is_empty() && other.documents.is_empty() && other.projects.is_empty());
}

#[tokio::test]
async fn knowledge_is_owner_scoped() {
    let s = state().await;
    let db = &s.db;

    let alice = kn::create_project(db, "alice", "Secret", "").await.unwrap();
    assert!(kn::get_project(db, "bob", &alice.uuid)
        .await
        .unwrap()
        .is_none());
    assert!(kn::list_projects(db, "bob").await.unwrap().is_empty());
    // Bob cannot make his idea a member of Alice's project (it's not his project).
    let bob_idea = ideas_repo::create_idea(db, "bob", "Hi", "", &[], "inbox", "draft", None)
        .await
        .unwrap();
    assert!(
        kn::set_membership(db, "bob", "idea", &bob_idea.uuid, Some(&alice.uuid))
            .await
            .is_err()
    );
}
