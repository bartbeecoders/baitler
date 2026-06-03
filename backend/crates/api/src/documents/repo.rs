//! SurrealDB persistence for documents. Owner-scoped; the route layer sanitizes
//! HTML bodies before they reach here.

use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};

use super::model::DocumentRow;

const DOC_SELECT: &str = "SELECT uuid, owner, title, body, version, review, project_id, tags, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM document";

fn vanished() -> AppError {
    AppError::Internal("document not found immediately after write".into())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_document(
    db: &Db,
    owner: &str,
    title: &str,
    body: &str,
    review: &str,
    project_id: Option<&str>,
    tags: &[String],
) -> AppResult<DocumentRow> {
    let uuid = Uuid::new_v4().to_string();
    let sql = format!(
        "CREATE document CONTENT {{ uuid: $uuid, owner: $owner, title: $title, body: $body, \
         review: $review, project_id: $project_id, tags: $tags }}; \
         {DOC_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("title", title.to_string()))
        .bind(("body", body.to_string()))
        .bind(("review", review.to_string()))
        .bind(("project_id", project_id.map(str::to_string)))
        .bind(("tags", tags.to_vec()))
        .await?
        .check()?;
    let rows: Vec<DocumentRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_document(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<DocumentRow>> {
    let sql = format!("{DOC_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    let rows: Vec<DocumentRow> = res.take(0)?;
    Ok(rows.into_iter().next())
}

pub async fn list_documents(
    db: &Db,
    owner: &str,
    tag: Option<&str>,
) -> AppResult<Vec<DocumentRow>> {
    let mut clauses = vec!["owner = $owner".to_string()];
    if tag.is_some() {
        clauses.push("$tag IN tags".to_string());
    }
    let sql = format!(
        "{DOC_SELECT} WHERE {} ORDER BY updated_at DESC",
        clauses.join(" AND ")
    );
    let mut query = db.query(sql).bind(("owner", owner.to_string()));
    if let Some(t) = tag {
        query = query.bind(("tag", t.to_string()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

/// Distinct tags used across this owner's documents.
pub async fn distinct_tags(db: &Db, owner: &str) -> AppResult<Vec<Vec<String>>> {
    let mut res = db
        .query("SELECT VALUE tags FROM document WHERE owner = $owner")
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

pub async fn update_document(
    db: &Db,
    owner: &str,
    uuid: &str,
    title: Option<&str>,
    body: Option<&str>,
    review: Option<&str>,
    tags: Option<&[String]>,
) -> AppResult<Option<DocumentRow>> {
    let mut sets: Vec<&str> = Vec::new();
    if title.is_some() {
        sets.push("title = $title");
    }
    if body.is_some() {
        sets.push("body = $body");
    }
    if review.is_some() {
        sets.push("review = $review");
    }
    if tags.is_some() {
        sets.push("tags = $tags");
    }
    if sets.is_empty() {
        return get_document(db, owner, uuid).await;
    }
    // Bump the version counter on a content edit (not on a review-only change,
    // so publishing doesn't masquerade as an edit).
    if title.is_some() || body.is_some() {
        sets.push("version = version + 1");
    }

    let sql = format!(
        "UPDATE document SET {} WHERE owner = $owner AND uuid = $uuid; \
         {DOC_SELECT} WHERE owner = $owner AND uuid = $uuid",
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
    if let Some(r) = review {
        query = query.bind(("review", r.to_string()));
    }
    if let Some(t) = tags {
        query = query.bind(("tags", t.to_vec()));
    }
    let mut res = query.await?.check()?;
    let rows: Vec<DocumentRow> = res.take(1)?;
    Ok(rows.into_iter().next())
}

pub async fn delete_document(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    if get_document(db, owner, uuid).await?.is_none() {
        return Ok(false);
    }
    // Scrub cross-type knowledge links (Phase 11) pointing at this document.
    crate::knowledge::repo::scrub_item_links(db, owner, "document", uuid).await?;
    db.query("DELETE document WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(true)
}
