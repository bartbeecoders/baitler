//! SurrealDB persistence for ideas. Every query is owner-scoped; timestamps are
//! projected to strings so DTOs serialize cleanly. Links are stored as an array
//! of related idea uuids and kept symmetric (undirected) by the link/unlink fns.

use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};

use super::model::IdeaRow;

const IDEA_SELECT: &str = "SELECT uuid, owner, title, body, tags, status, links, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM idea";

fn vanished() -> AppError {
    AppError::Internal("idea not found immediately after write".into())
}

pub async fn create_idea(
    db: &Db,
    owner: &str,
    title: &str,
    body: &str,
    tags: &[String],
    status: &str,
) -> AppResult<IdeaRow> {
    let uuid = Uuid::new_v4().to_string();
    let sql = format!(
        "CREATE idea CONTENT {{ uuid: $uuid, owner: $owner, title: $title, body: $body, \
         tags: $tags, status: $status, links: [] }}; \
         {IDEA_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("title", title.to_string()))
        .bind(("body", body.to_string()))
        .bind(("tags", tags.to_vec()))
        .bind(("status", status.to_string()))
        .await?
        .check()?;
    let rows: Vec<IdeaRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_idea(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<IdeaRow>> {
    let sql = format!("{IDEA_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    let rows: Vec<IdeaRow> = res.take(0)?;
    Ok(rows.into_iter().next())
}

pub async fn list_ideas(
    db: &Db,
    owner: &str,
    status: Option<&str>,
    tag: Option<&str>,
    q: Option<&str>,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<IdeaRow>> {
    let mut clauses = vec!["owner = $owner".to_string()];
    if status.is_some() {
        clauses.push("status = $status".to_string());
    }
    if tag.is_some() {
        clauses.push("$tag IN tags".to_string());
    }
    if q.is_some() {
        clauses.push(
            "(string::lowercase(title) CONTAINS string::lowercase($q) \
             OR string::lowercase(body) CONTAINS string::lowercase($q))"
                .to_string(),
        );
    }
    let sql = format!(
        "{IDEA_SELECT} WHERE {} ORDER BY updated_at DESC LIMIT $limit START $offset",
        clauses.join(" AND ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64));
    if let Some(s) = status {
        query = query.bind(("status", s.to_string()));
    }
    if let Some(t) = tag {
        query = query.bind(("tag", t.to_string()));
    }
    if let Some(query_str) = q {
        query = query.bind(("q", query_str.to_string()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

pub async fn update_idea(
    db: &Db,
    owner: &str,
    uuid: &str,
    title: Option<&str>,
    body: Option<&str>,
    tags: Option<&[String]>,
    status: Option<&str>,
) -> AppResult<Option<IdeaRow>> {
    let mut sets: Vec<&str> = Vec::new();
    if title.is_some() {
        sets.push("title = $title");
    }
    if body.is_some() {
        sets.push("body = $body");
    }
    if tags.is_some() {
        sets.push("tags = $tags");
    }
    if status.is_some() {
        sets.push("status = $status");
    }
    if sets.is_empty() {
        return get_idea(db, owner, uuid).await;
    }

    let sql = format!(
        "UPDATE idea SET {} WHERE owner = $owner AND uuid = $uuid; \
         {IDEA_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(t) = title {
        query = query.bind(("title", t.to_string()));
    }
    if let Some(b) = body {
        query = query.bind(("body", b.to_string()));
    }
    if let Some(t) = tags {
        query = query.bind(("tags", t.to_vec()));
    }
    if let Some(s) = status {
        query = query.bind(("status", s.to_string()));
    }
    let mut res = query.await?.check()?;
    let rows: Vec<IdeaRow> = res.take(1)?;
    Ok(rows.into_iter().next())
}

/// Overwrite an idea's `links` array.
async fn set_links(db: &Db, owner: &str, uuid: &str, links: &[String]) -> AppResult<()> {
    db.query("UPDATE idea SET links = $links WHERE owner = $owner AND uuid = $uuid")
        .bind(("links", links.to_vec()))
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(())
}

/// Create a symmetric link between two ideas (both must exist and be owned).
pub async fn link_ideas(db: &Db, owner: &str, a: &str, b: &str) -> AppResult<()> {
    let idea_a = get_idea(db, owner, a).await?.ok_or(AppError::NotFound)?;
    let idea_b = get_idea(db, owner, b)
        .await?
        .ok_or_else(|| AppError::BadRequest("target idea does not exist".into()))?;

    let mut a_links = idea_a.links;
    if !a_links.iter().any(|l| l == b) {
        a_links.push(b.to_string());
    }
    let mut b_links = idea_b.links;
    if !b_links.iter().any(|l| l == a) {
        b_links.push(a.to_string());
    }
    set_links(db, owner, a, &a_links).await?;
    set_links(db, owner, b, &b_links).await?;
    Ok(())
}

/// Remove a symmetric link between two ideas. No-op if not linked.
pub async fn unlink_ideas(db: &Db, owner: &str, a: &str, b: &str) -> AppResult<()> {
    if let Some(idea_a) = get_idea(db, owner, a).await? {
        let a_links: Vec<String> = idea_a.links.into_iter().filter(|l| l != b).collect();
        set_links(db, owner, a, &a_links).await?;
    }
    if let Some(idea_b) = get_idea(db, owner, b).await? {
        let b_links: Vec<String> = idea_b.links.into_iter().filter(|l| l != a).collect();
        set_links(db, owner, b, &b_links).await?;
    }
    Ok(())
}

/// Delete an idea (scrubbing its id from linked ideas first). Returns whether it
/// existed.
pub async fn delete_idea(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    let Some(idea) = get_idea(db, owner, uuid).await? else {
        return Ok(false);
    };
    for linked in &idea.links {
        if let Some(other) = get_idea(db, owner, linked).await? {
            let scrubbed: Vec<String> = other.links.into_iter().filter(|l| l != uuid).collect();
            set_links(db, owner, linked, &scrubbed).await?;
        }
    }
    db.query("DELETE idea WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(true)
}

/// Resolve a set of idea uuids to rows (owner-scoped; unknown ids are skipped).
pub async fn resolve_many(db: &Db, owner: &str, ids: &[String]) -> AppResult<Vec<IdeaRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!("{IDEA_SELECT} WHERE owner = $owner AND uuid IN $ids");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("ids", ids.to_vec()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

/// The distinct, sorted set of tags used across the owner's ideas.
pub async fn distinct_tags(db: &Db, owner: &str) -> AppResult<Vec<String>> {
    let mut res = db
        .query("SELECT VALUE tags FROM idea WHERE owner = $owner")
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    let per_idea: Vec<Vec<String>> = res.take(0)?;
    let mut tags: Vec<String> = per_idea.into_iter().flatten().collect();
    tags.sort();
    tags.dedup();
    Ok(tags)
}
