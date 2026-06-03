//! SurrealDB persistence for draw.io diagrams. Owner-scoped. The mxGraph `xml`
//! is stored verbatim; a derived `search_text` (title + extracted labels) feeds
//! the full-text index. The optional `preview` is a sanitized SVG/PNG data URI.

use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::knowledge::repo as kn;

use super::model::{extract_labels, DiagramRow};

const DIAGRAM_SELECT: &str = "SELECT uuid, owner, title, xml, preview, folder_id, project_id, \
    tags, review, version, \
    IF published_at = NONE THEN NONE ELSE type::string(published_at) END AS published_at, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM diagram";

fn vanished() -> AppError {
    AppError::Internal("diagram not found immediately after write".into())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_diagram(
    db: &Db,
    owner: &str,
    title: &str,
    xml: &str,
    preview: &str,
    folder_id: Option<&str>,
    project_id: Option<&str>,
    tags: &[String],
    review: &str,
) -> AppResult<DiagramRow> {
    let uuid = Uuid::new_v4().to_string();
    let search_text = format!("{title} {}", extract_labels(xml));
    let sql = format!(
        "CREATE diagram CONTENT {{ uuid: $uuid, owner: $owner, title: $title, xml: $xml, \
         preview: $preview, search_text: $search_text, folder_id: $folder_id, \
         project_id: $project_id, tags: $tags, review: $review }}; \
         {DIAGRAM_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("title", title.to_string()))
        .bind(("xml", xml.to_string()))
        .bind(("preview", preview.to_string()))
        .bind(("search_text", search_text))
        .bind(("folder_id", folder_id.map(str::to_string)))
        .bind(("project_id", project_id.map(str::to_string)))
        .bind(("tags", tags.to_vec()))
        .bind(("review", review.to_string()))
        .await?
        .check()?;
    let rows: Vec<DiagramRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_diagram(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<DiagramRow>> {
    let sql = format!("{DIAGRAM_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<DiagramRow>>(0)?.into_iter().next())
}

#[allow(clippy::too_many_arguments)]
pub async fn list_diagrams(
    db: &Db,
    owner: &str,
    folder: Option<&str>,
    project: Option<&str>,
    tag: Option<&str>,
    review: Option<&str>,
    q: Option<&str>,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<DiagramRow>> {
    let mut clauses = vec!["owner = $owner".to_string()];
    if folder.is_some() {
        clauses.push("folder_id = $folder".to_string());
    }
    if project.is_some() {
        clauses.push("project_id = $project".to_string());
    }
    if tag.is_some() {
        clauses.push("$tag IN tags".to_string());
    }
    if review.is_some() {
        clauses.push("review = $review".to_string());
    }
    if q.is_some() {
        clauses.push(
            "(string::lowercase(title) CONTAINS string::lowercase($q) \
             OR string::lowercase(search_text) CONTAINS string::lowercase($q))"
                .to_string(),
        );
    }
    let sql = format!(
        "{DIAGRAM_SELECT} WHERE {} ORDER BY updated_at DESC LIMIT $limit START $offset",
        clauses.join(" AND ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64));
    if let Some(f) = folder {
        query = query.bind(("folder", f.to_string()));
    }
    if let Some(p) = project {
        query = query.bind(("project", p.to_string()));
    }
    if let Some(t) = tag {
        query = query.bind(("tag", t.to_string()));
    }
    if let Some(r) = review {
        query = query.bind(("review", r.to_string()));
    }
    if let Some(s) = q {
        query = query.bind(("q", s.to_string()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

/// Distinct tags used across this owner's diagrams.
pub async fn distinct_tags(db: &Db, owner: &str) -> AppResult<Vec<Vec<String>>> {
    let mut res = db
        .query("SELECT VALUE tags FROM diagram WHERE owner = $owner")
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

/// A partial update. `folder_id`/`project_id` use a double-option:
/// `None` = leave, `Some(None)` = clear, `Some(Some(id))` = set.
#[derive(Default)]
pub struct DiagramPatch<'a> {
    pub title: Option<&'a str>,
    pub xml: Option<&'a str>,
    pub preview: Option<&'a str>,
    pub review: Option<&'a str>,
    pub folder_id: Option<Option<&'a str>>,
    pub project_id: Option<Option<&'a str>>,
    pub tags: Option<&'a [String]>,
}

pub async fn update_diagram(
    db: &Db,
    owner: &str,
    uuid: &str,
    patch: DiagramPatch<'_>,
) -> AppResult<Option<DiagramRow>> {
    let Some(current) = get_diagram(db, owner, uuid).await? else {
        return Ok(None);
    };

    let mut sets: Vec<&str> = Vec::new();
    if patch.title.is_some() {
        sets.push("title = $title");
    }
    if patch.xml.is_some() {
        sets.push("xml = $xml");
    }
    if patch.preview.is_some() {
        sets.push("preview = $preview");
    }
    // search_text tracks title + (possibly new) xml labels.
    if patch.title.is_some() || patch.xml.is_some() {
        sets.push("search_text = $search_text");
    }
    if patch.review.is_some() {
        sets.push("review = $review");
    }
    match patch.folder_id {
        Some(Some(_)) => sets.push("folder_id = $folder"),
        Some(None) => sets.push("folder_id = NONE"),
        None => {}
    }
    match patch.project_id {
        Some(Some(_)) => sets.push("project_id = $project"),
        Some(None) => sets.push("project_id = NONE"),
        None => {}
    }
    if patch.tags.is_some() {
        sets.push("tags = $tags");
    }
    // Version bumps on a content edit only (title or xml).
    if patch.title.is_some() || patch.xml.is_some() {
        sets.push("version = version + 1");
    }
    if sets.is_empty() {
        return Ok(Some(current));
    }

    let new_title = patch.title.unwrap_or(&current.title);
    let xml_for_labels = patch.xml.unwrap_or(&current.xml);
    let search_text = format!("{new_title} {}", extract_labels(xml_for_labels));

    let sql = format!(
        "UPDATE diagram SET {} WHERE owner = $owner AND uuid = $uuid; \
         {DIAGRAM_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(t) = patch.title {
        query = query.bind(("title", t.to_string()));
    }
    if let Some(x) = patch.xml {
        query = query.bind(("xml", x.to_string()));
    }
    if let Some(p) = patch.preview {
        query = query.bind(("preview", p.to_string()));
    }
    if patch.title.is_some() || patch.xml.is_some() {
        query = query.bind(("search_text", search_text));
    }
    if let Some(r) = patch.review {
        query = query.bind(("review", r.to_string()));
    }
    if let Some(Some(f)) = patch.folder_id {
        query = query.bind(("folder", f.to_string()));
    }
    if let Some(Some(p)) = patch.project_id {
        query = query.bind(("project", p.to_string()));
    }
    if let Some(t) = patch.tags {
        query = query.bind(("tags", t.to_vec()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take::<Vec<DiagramRow>>(1)?.into_iter().next())
}

/// Delete a diagram, scrubbing its cross-type knowledge links first.
pub async fn delete_diagram(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    if get_diagram(db, owner, uuid).await?.is_none() {
        return Ok(false);
    }
    kn::scrub_item_links(db, owner, "diagram", uuid).await?;
    db.query("DELETE diagram WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(true)
}
