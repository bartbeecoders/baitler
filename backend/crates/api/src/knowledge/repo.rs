//! SurrealDB persistence for the knowledge layer. Every query is owner-scoped.
//!
//! Membership = the `project_id` pointer on idea/document/file. Links = symmetric
//! `kn_link` edge rows (written in BOTH directions so backlinks resolve from
//! either endpoint and the `kn_link_dst` index always applies), mirroring the
//! Phase 5 `link_ideas` precedent. Table names are interpolated only from the
//! validated `ITEM_TYPES`/`MEMBER_TYPES` whitelists — never from raw input.

use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};

use super::model::{
    LinkRef, LinkRow, MemberCounts, MemberItem, ProjectMembers, ProjectRow, ReviewQueue, SearchHit,
    SearchResults, ITEM_TYPES, MEMBER_TYPES,
};

const PROJECT_SELECT: &str = "SELECT uuid, owner, name, slug, summary, status, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM project";

fn vanished() -> AppError {
    AppError::Internal("project not found immediately after write".into())
}

/// Validate a type token against a whitelist, returning the safe table name.
fn member_table(kind: &str) -> AppResult<&'static str> {
    MEMBER_TYPES
        .iter()
        .copied()
        .find(|t| *t == kind)
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "invalid item type `{kind}` (expected one of: {})",
                MEMBER_TYPES.join(", ")
            ))
        })
}

fn any_table(kind: &str) -> AppResult<&'static str> {
    ITEM_TYPES
        .iter()
        .copied()
        .find(|t| *t == kind)
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "invalid item type `{kind}` (expected one of: {})",
                ITEM_TYPES.join(", ")
            ))
        })
}

/// The title-ish column for a table (`name` for files/projects, `title` else).
fn title_col(table: &str) -> &'static str {
    match table {
        "file" | "project" => "name",
        _ => "title",
    }
}

// ── Projects ──────────────────────────────────────────────────────────────────

pub async fn create_project(
    db: &Db,
    owner: &str,
    name: &str,
    summary: &str,
) -> AppResult<ProjectRow> {
    let uuid = Uuid::new_v4().to_string();
    let slug =
        crate::slug::unique_slug(db, owner, "project", &crate::slug::slugify(name, "project"))
            .await?;
    let sql = format!(
        "CREATE project CONTENT {{ uuid: $uuid, owner: $owner, name: $name, slug: $slug, \
         summary: $summary, status: \"active\" }}; \
         {PROJECT_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("name", name.to_string()))
        .bind(("slug", slug))
        .bind(("summary", summary.to_string()))
        .await?
        .check()?;
    let rows: Vec<ProjectRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_project(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<ProjectRow>> {
    let sql = format!("{PROJECT_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    let rows: Vec<ProjectRow> = res.take(0)?;
    Ok(rows.into_iter().next())
}

pub async fn list_projects(db: &Db, owner: &str) -> AppResult<Vec<ProjectRow>> {
    let sql = format!("{PROJECT_SELECT} WHERE owner = $owner ORDER BY updated_at DESC");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

pub async fn update_project(
    db: &Db,
    owner: &str,
    uuid: &str,
    name: Option<&str>,
    summary: Option<&str>,
    status: Option<&str>,
) -> AppResult<Option<ProjectRow>> {
    let mut sets: Vec<&str> = Vec::new();
    if name.is_some() {
        sets.push("name = $name");
    }
    if summary.is_some() {
        sets.push("summary = $summary");
    }
    if status.is_some() {
        sets.push("status = $status");
    }
    if sets.is_empty() {
        return get_project(db, owner, uuid).await;
    }
    let sql = format!(
        "UPDATE project SET {} WHERE owner = $owner AND uuid = $uuid; \
         {PROJECT_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(n) = name {
        query = query.bind(("name", n.to_string()));
    }
    if let Some(s) = summary {
        query = query.bind(("summary", s.to_string()));
    }
    if let Some(s) = status {
        query = query.bind(("status", s.to_string()));
    }
    let mut res = query.await?.check()?;
    let rows: Vec<ProjectRow> = res.take(1)?;
    Ok(rows.into_iter().next())
}

/// Delete a project: detach its members (clear their `project_id`), scrub its
/// links, then remove it. The member items themselves are never deleted.
pub async fn delete_project(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    if get_project(db, owner, uuid).await?.is_none() {
        return Ok(false);
    }
    for table in MEMBER_TYPES {
        db.query(format!(
            "UPDATE {table} SET project_id = NONE WHERE owner = $owner AND project_id = $pid"
        ))
        .bind(("owner", owner.to_string()))
        .bind(("pid", uuid.to_string()))
        .await?
        .check()?;
    }
    scrub_item_links(db, owner, "project", uuid).await?;
    db.query("DELETE project WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(true)
}

// ── Membership ────────────────────────────────────────────────────────────────

/// Set (or clear, with `None`) an item's project membership. Validates that the
/// item — and the target project, if any — exist and are owned.
pub async fn set_membership(
    db: &Db,
    owner: &str,
    kind: &str,
    id: &str,
    project_id: Option<&str>,
) -> AppResult<()> {
    let table = member_table(kind)?;
    if !item_exists(db, owner, kind, id).await? {
        return Err(AppError::NotFound);
    }
    if let Some(pid) = project_id {
        if get_project(db, owner, pid).await?.is_none() {
            return Err(AppError::BadRequest("target project does not exist".into()));
        }
    }
    db.query(format!(
        "UPDATE {table} SET project_id = $pid WHERE owner = $owner AND uuid = $id"
    ))
    .bind(("owner", owner.to_string()))
    .bind(("id", id.to_string()))
    .bind(("pid", project_id.map(str::to_string)))
    .await?
    .check()?;
    Ok(())
}

/// Members of a project grouped by type, newest first.
pub async fn project_members(db: &Db, owner: &str, project_id: &str) -> AppResult<ProjectMembers> {
    Ok(ProjectMembers {
        ideas: members_of(db, owner, "idea", project_id).await?,
        documents: members_of(db, owner, "document", project_id).await?,
        files: members_of(db, owner, "file", project_id).await?,
        pages: members_of(db, owner, "page", project_id).await?,
        mindmaps: members_of(db, owner, "mindmap", project_id).await?,
        diagrams: members_of(db, owner, "diagram", project_id).await?,
        superpages: members_of(db, owner, "superpage", project_id).await?,
    })
}

async fn members_of(
    db: &Db,
    owner: &str,
    table: &str,
    project_id: &str,
) -> AppResult<Vec<MemberItem>> {
    let tcol = title_col(table);
    // Files and pages have no `review` field (pages gate on `visibility`); project
    // the column as NONE so the member shape is uniform across types.
    let review_expr = if table == "file" || table == "page" {
        "NONE"
    } else {
        "review"
    };
    // `updated_at` is projected only so it can drive ORDER BY (SurrealDB requires
    // the ordered idiom to be selected); `MemberItem` deserialization ignores it.
    let sql = format!(
        "SELECT uuid AS id, {tcol} AS title, {review_expr} AS review, updated_at FROM {table} \
         WHERE owner = $owner AND project_id = $pid ORDER BY updated_at DESC"
    );
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("pid", project_id.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

/// Per-type counts (plus pending-draft count) for a project.
pub async fn member_counts(db: &Db, owner: &str, project_id: &str) -> AppResult<MemberCounts> {
    let members = project_members(db, owner, project_id).await?;
    let drafts = members
        .ideas
        .iter()
        .chain(members.documents.iter())
        .filter(|m| m.review.as_deref() == Some("draft"))
        .count();
    Ok(MemberCounts {
        ideas: members.ideas.len(),
        documents: members.documents.len(),
        files: members.files.len(),
        pages: members.pages.len(),
        mindmaps: members.mindmaps.len(),
        diagrams: members.diagrams.len(),
        superpages: members.superpages.len(),
        drafts,
    })
}

// ── Review queue ──────────────────────────────────────────────────────────────

/// Ideas + documents still in `review = "draft"`, newest first — what the portal
/// review queue shows for human approval.
pub async fn review_queue(db: &Db, owner: &str) -> AppResult<ReviewQueue> {
    Ok(ReviewQueue {
        ideas: drafts_of(db, owner, "idea").await?,
        documents: drafts_of(db, owner, "document").await?,
    })
}

async fn drafts_of(db: &Db, owner: &str, table: &str) -> AppResult<Vec<MemberItem>> {
    let sql = format!(
        "SELECT uuid AS id, title, review, updated_at FROM {table} \
         WHERE owner = $owner AND review = \"draft\" ORDER BY updated_at DESC"
    );
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

// ── Cross-type links ──────────────────────────────────────────────────────────

/// Whether an owned item of the given type exists.
pub async fn item_exists(db: &Db, owner: &str, kind: &str, id: &str) -> AppResult<bool> {
    let table = any_table(kind)?;
    let mut res = db
        .query(format!(
            "SELECT VALUE uuid FROM {table} WHERE owner = $owner AND uuid = $id"
        ))
        .bind(("owner", owner.to_string()))
        .bind(("id", id.to_string()))
        .await?
        .check()?;
    let found: Vec<String> = res.take(0)?;
    Ok(!found.is_empty())
}

/// Best-effort display title/name for an item (None if missing).
pub async fn resolve_title(db: &Db, owner: &str, kind: &str, id: &str) -> AppResult<Option<String>> {
    let table = any_table(kind)?;
    let tcol = title_col(table);
    let mut res = db
        .query(format!(
            "SELECT VALUE {tcol} FROM {table} WHERE owner = $owner AND uuid = $id"
        ))
        .bind(("owner", owner.to_string()))
        .bind(("id", id.to_string()))
        .await?
        .check()?;
    let titles: Vec<String> = res.take(0)?;
    Ok(titles.into_iter().next())
}

/// Create a symmetric link between two items (both must exist and be owned).
/// Idempotent: an existing pair is replaced (so `relation` can be updated).
pub async fn link_items(
    db: &Db,
    owner: &str,
    a_type: &str,
    a_id: &str,
    b_type: &str,
    b_id: &str,
    relation: &str,
) -> AppResult<()> {
    any_table(a_type)?;
    any_table(b_type)?;
    if a_type == b_type && a_id == b_id {
        return Err(AppError::BadRequest("an item cannot link to itself".into()));
    }
    if !item_exists(db, owner, a_type, a_id).await? {
        return Err(AppError::NotFound);
    }
    if !item_exists(db, owner, b_type, b_id).await? {
        return Err(AppError::BadRequest("link target does not exist".into()));
    }
    // Replace any existing pair so the call is idempotent and relation-updating.
    unlink_items(db, owner, a_type, a_id, b_type, b_id).await?;
    for (st, si, dt, di) in [(a_type, a_id, b_type, b_id), (b_type, b_id, a_type, a_id)] {
        db.query(
            "CREATE kn_link CONTENT { uuid: $uuid, owner: $owner, src_type: $st, src_id: $si, \
             dst_type: $dt, dst_id: $di, relation: $rel }",
        )
        .bind(("uuid", Uuid::new_v4().to_string()))
        .bind(("owner", owner.to_string()))
        .bind(("st", st.to_string()))
        .bind(("si", si.to_string()))
        .bind(("dt", dt.to_string()))
        .bind(("di", di.to_string()))
        .bind(("rel", relation.to_string()))
        .await?
        .check()?;
    }
    Ok(())
}

/// Remove the symmetric link between two items (both directions). No-op if absent.
pub async fn unlink_items(
    db: &Db,
    owner: &str,
    a_type: &str,
    a_id: &str,
    b_type: &str,
    b_id: &str,
) -> AppResult<()> {
    db.query(
        "DELETE kn_link WHERE owner = $owner AND ( \
           (src_type = $at AND src_id = $aid AND dst_type = $bt AND dst_id = $bid) OR \
           (src_type = $bt AND src_id = $bid AND dst_type = $at AND dst_id = $aid))",
    )
    .bind(("owner", owner.to_string()))
    .bind(("at", a_type.to_string()))
    .bind(("aid", a_id.to_string()))
    .bind(("bt", b_type.to_string()))
    .bind(("bid", b_id.to_string()))
    .await?
    .check()?;
    Ok(())
}

/// All items linked to `(kind, id)`, resolved to typed references with titles.
pub async fn backlinks(db: &Db, owner: &str, kind: &str, id: &str) -> AppResult<Vec<LinkRef>> {
    any_table(kind)?;
    let mut res = db
        .query(
            "SELECT * FROM kn_link WHERE owner = $owner AND src_type = $t AND src_id = $id \
             ORDER BY created_at DESC",
        )
        .bind(("owner", owner.to_string()))
        .bind(("t", kind.to_string()))
        .bind(("id", id.to_string()))
        .await?
        .check()?;
    let rows: Vec<LinkRow> = res.take(0)?;

    let mut refs = Vec::with_capacity(rows.len());
    for row in rows {
        let title = resolve_title(db, owner, &row.dst_type, &row.dst_id).await?;
        refs.push(LinkRef {
            item_type: row.dst_type,
            id: row.dst_id,
            title,
            relation: row.relation,
        });
    }
    Ok(refs)
}

/// Delete every link touching `(kind, id)` in either direction. Called from the
/// item delete paths so a removed idea/document/file/project leaves no dangling
/// edges — mirroring the Phase 5 link-scrub-on-delete precedent.
pub async fn scrub_item_links(db: &Db, owner: &str, kind: &str, id: &str) -> AppResult<()> {
    db.query(
        "DELETE kn_link WHERE owner = $owner AND \
         ((src_type = $t AND src_id = $id) OR (dst_type = $t AND dst_id = $id))",
    )
    .bind(("owner", owner.to_string()))
    .bind(("t", kind.to_string()))
    .bind(("id", id.to_string()))
    .await?
    .check()?;
    Ok(())
}

// ── Cross-type full-text search ───────────────────────────────────────────────

/// Search ideas, documents, projects, files, and pages by full text. Returns
/// typed sections, each ranked by BM25 score then recency. Owner-scoped; backed
/// by the per-field SEARCH indexes from migrations 0008 (idea/document/project/
/// file) and 0010 (page).
pub async fn search(db: &Db, owner: &str, q: &str, limit: usize) -> AppResult<SearchResults> {
    Ok(SearchResults {
        ideas: search_two(db, owner, "idea", "title", "body", q, limit).await?,
        documents: search_two(db, owner, "document", "title", "body", q, limit).await?,
        projects: search_two(db, owner, "project", "name", "summary", q, limit).await?,
        files: search_one(db, owner, "file", "name", q, limit).await?,
        pages: search_two(db, owner, "page", "title", "body", q, limit).await?,
        mindmaps: search_two(db, owner, "mindmap", "title", "search_text", q, limit).await?,
        diagrams: search_two(db, owner, "diagram", "title", "search_text", q, limit).await?,
        superpages: search_two(db, owner, "superpage", "title", "search_text", q, limit).await?,
    })
}

/// Cross-type tag taxonomy: every distinct tag used across this owner's ideas,
/// documents, and pages with how many items carry it, ordered by count then name.
/// Table names are trusted literals.
pub async fn tag_counts(db: &Db, owner: &str) -> AppResult<Vec<super::model::TagCount>> {
    use std::collections::HashMap;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for table in ["idea", "document", "page"] {
        let sql = format!("SELECT VALUE tags FROM {table} WHERE owner = $owner");
        let mut res = db
            .query(sql)
            .bind(("owner", owner.to_string()))
            .await?
            .check()?;
        let per_row: Vec<Vec<String>> = res.take(0)?;
        for tags in per_row {
            for tag in tags {
                *counts.entry(tag).or_insert(0) += 1;
            }
        }
    }
    let mut out: Vec<super::model::TagCount> = counts
        .into_iter()
        .map(|(tag, count)| super::model::TagCount { tag, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    Ok(out)
}

/// Search a table whose text lives in two fields (`@0@` / `@1@`). The snippet
/// highlights the second field (the "body"); the score sums both predicates.
async fn search_two(
    db: &Db,
    owner: &str,
    table: &str,
    f0: &str,
    f1: &str,
    q: &str,
    limit: usize,
) -> AppResult<Vec<SearchHit>> {
    let tcol = title_col(table);
    let sql = format!(
        "SELECT uuid AS id, {tcol} AS title, \
                search::highlight('<mark>', '</mark>', 1) AS snippet, \
                (search::score(0) ?? 0) + (search::score(1) ?? 0) AS score, updated_at \
         FROM {table} WHERE owner = $owner AND ({f0} @0@ $q OR {f1} @1@ $q) \
         ORDER BY score DESC, updated_at DESC LIMIT $limit"
    );
    run_search(db, owner, sql, q, limit).await
}

/// Search a table whose text lives in a single field (`@0@`).
async fn search_one(
    db: &Db,
    owner: &str,
    table: &str,
    f0: &str,
    q: &str,
    limit: usize,
) -> AppResult<Vec<SearchHit>> {
    let tcol = title_col(table);
    let sql = format!(
        "SELECT uuid AS id, {tcol} AS title, \
                search::highlight('<mark>', '</mark>', 0) AS snippet, \
                (search::score(0) ?? 0) AS score, updated_at \
         FROM {table} WHERE owner = $owner AND {f0} @0@ $q \
         ORDER BY score DESC, updated_at DESC LIMIT $limit"
    );
    run_search(db, owner, sql, q, limit).await
}

async fn run_search(
    db: &Db,
    owner: &str,
    sql: String,
    q: &str,
    limit: usize,
) -> AppResult<Vec<SearchHit>> {
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("q", q.to_string()))
        .bind(("limit", limit as i64))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

// Slug generation lives in `crate::slug` (shared with pages, Phase 12).
