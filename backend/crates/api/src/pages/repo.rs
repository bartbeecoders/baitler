//! SurrealDB persistence for hosted pages. Owner-scoped (except the public
//! serve lookup). The `body` is sanitized on every write so no draft/preview/
//! search path ever holds unsanitized markup; the public serve path additionally
//! runs `convert::harden_for_render` (Phase 12.5).

use uuid::Uuid;

use crate::convert;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::slug;

use super::model::PageRow;

const PAGE_SELECT: &str = "SELECT uuid, owner, title, body, slug, visibility, source_format, \
    folder_id, project_id, version, tags, \
    IF published_at = NONE THEN NONE ELSE type::string(published_at) END AS published_at, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM page";

fn vanished() -> AppError {
    AppError::Internal("page not found immediately after write".into())
}

/// Normalize authored content to sanitized HTML: Markdown is rendered first,
/// then everything is run through `ammonia` (scripts/handlers stripped).
fn render_body(raw: &str, source_format: &str) -> String {
    let html = if source_format == "markdown" {
        convert::md_to_html(raw)
    } else {
        raw.to_string()
    };
    convert::sanitize(&html)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_page(
    db: &Db,
    owner: &str,
    title: &str,
    raw_body: &str,
    source_format: &str,
    visibility: &str,
    folder_id: Option<&str>,
    project_id: Option<&str>,
    tags: &[String],
) -> AppResult<PageRow> {
    let uuid = Uuid::new_v4().to_string();
    // Unlisted pages get unguessable slug entropy (the slug IS the access token).
    let mut base = slug::slugify(title, "page");
    if visibility == "unlisted" {
        base = slug::with_entropy(&base);
    }
    let slug = slug::unique_slug(db, owner, "page", &base).await?;
    let body = render_body(raw_body, source_format);
    // `visibility` is validated against VISIBILITIES by the caller, so this
    // literal-expression interpolation is safe.
    let published_expr = if visibility == "draft" {
        "NONE"
    } else {
        "time::now()"
    };

    let sql = format!(
        "CREATE page CONTENT {{ uuid: $uuid, owner: $owner, title: $title, body: $body, \
         slug: $slug, visibility: $visibility, source_format: $source_format, \
         folder_id: $folder_id, project_id: $project_id, tags: $tags, \
         published_at: {published_expr} }}; \
         {PAGE_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("title", title.to_string()))
        .bind(("body", body))
        .bind(("slug", slug))
        .bind(("visibility", visibility.to_string()))
        .bind(("source_format", source_format.to_string()))
        .bind(("folder_id", folder_id.map(str::to_string)))
        .bind(("project_id", project_id.map(str::to_string)))
        .bind(("tags", tags.to_vec()))
        .await?
        .check()?;
    let rows: Vec<PageRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_page(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<PageRow>> {
    let sql = format!("{PAGE_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<PageRow>>(0)?.into_iter().next())
}

/// Owner-scoped slug lookup (management). The unauthenticated public serve
/// lookup is [`get_served_by_slug`].
pub async fn get_page_by_slug(db: &Db, owner: &str, slug: &str) -> AppResult<Option<PageRow>> {
    let sql = format!("{PAGE_SELECT} WHERE owner = $owner AND slug = $slug");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("slug", slug.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<PageRow>>(0)?.into_iter().next())
}

/// **Unauthenticated** lookup for public serving (Phase 12.5): resolves a slug
/// to a page only when it is `unlisted` or `public`. Deliberately NOT
/// owner-scoped — a published page is public — and never returns `draft` rows
/// (so `GET /p/{slug}` 404s for drafts without confirming existence).
pub async fn get_served_by_slug(db: &Db, slug: &str) -> AppResult<Option<PageRow>> {
    let sql =
        format!("{PAGE_SELECT} WHERE slug = $slug AND visibility IN [\"unlisted\", \"public\"]");
    let mut res = db
        .query(sql)
        .bind(("slug", slug.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<PageRow>>(0)?.into_iter().next())
}

#[allow(clippy::too_many_arguments)]
pub async fn list_pages(
    db: &Db,
    owner: &str,
    folder: Option<&str>,
    visibility: Option<&str>,
    project: Option<&str>,
    tag: Option<&str>,
    q: Option<&str>,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<PageRow>> {
    let mut clauses = vec!["owner = $owner".to_string()];
    if folder.is_some() {
        clauses.push("folder_id = $folder".to_string());
    }
    if visibility.is_some() {
        clauses.push("visibility = $visibility".to_string());
    }
    if project.is_some() {
        clauses.push("project_id = $project".to_string());
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
        "{PAGE_SELECT} WHERE {} ORDER BY updated_at DESC LIMIT $limit START $offset",
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
    if let Some(v) = visibility {
        query = query.bind(("visibility", v.to_string()));
    }
    if let Some(p) = project {
        query = query.bind(("project", p.to_string()));
    }
    if let Some(t) = tag {
        query = query.bind(("tag", t.to_string()));
    }
    if let Some(s) = q {
        query = query.bind(("q", s.to_string()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

/// Distinct tags used across this owner's pages.
pub async fn distinct_tags(db: &Db, owner: &str) -> AppResult<Vec<Vec<String>>> {
    let mut res = db
        .query("SELECT VALUE tags FROM page WHERE owner = $owner")
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

/// A partial update to a page. `folder_id`/`project_id` use a double-option:
/// `None` = leave, `Some(None)` = clear, `Some(Some(id))` = set.
#[derive(Default)]
pub struct PagePatch<'a> {
    pub title: Option<&'a str>,
    /// Raw authored body; re-rendered + sanitized with `source_format`.
    pub body: Option<&'a str>,
    pub source_format: Option<&'a str>,
    pub slug: Option<&'a str>,
    pub visibility: Option<&'a str>,
    pub folder_id: Option<Option<&'a str>>,
    pub project_id: Option<Option<&'a str>>,
    pub tags: Option<&'a [String]>,
}

pub async fn update_page(
    db: &Db,
    owner: &str,
    uuid: &str,
    patch: PagePatch<'_>,
) -> AppResult<Option<PageRow>> {
    let Some(current) = get_page(db, owner, uuid).await? else {
        return Ok(None);
    };

    let mut sets: Vec<&str> = Vec::new();
    if patch.title.is_some() {
        sets.push("title = $title");
    }
    // Body re-render uses the new source_format if given, else the page's current one.
    let rendered_body = patch.body.map(|raw| {
        let fmt = patch.source_format.unwrap_or(&current.source_format);
        render_body(raw, fmt)
    });
    if rendered_body.is_some() {
        sets.push("body = $body");
    }
    if patch.source_format.is_some() {
        sets.push("source_format = $source_format");
    }
    if patch.slug.is_some() {
        sets.push("slug = $slug");
    }
    if let Some(v) = patch.visibility {
        sets.push("visibility = $visibility");
        if v == "draft" {
            sets.push("published_at = NONE");
        } else if current.visibility == "draft" {
            // First publish (draft → unlisted/public) stamps the time.
            sets.push("published_at = time::now()");
        }
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
    // Version bumps on a content edit only (not a visibility/move/slug/tag change).
    if patch.title.is_some() || rendered_body.is_some() {
        sets.push("version = version + 1");
    }
    if sets.is_empty() {
        return Ok(Some(current));
    }

    let sql = format!(
        "UPDATE page SET {} WHERE owner = $owner AND uuid = $uuid; \
         {PAGE_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(t) = patch.title {
        query = query.bind(("title", t.to_string()));
    }
    if let Some(b) = rendered_body {
        query = query.bind(("body", b));
    }
    if let Some(f) = patch.source_format {
        query = query.bind(("source_format", f.to_string()));
    }
    if let Some(s) = patch.slug {
        query = query.bind(("slug", s.to_string()));
    }
    if let Some(v) = patch.visibility {
        query = query.bind(("visibility", v.to_string()));
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
    Ok(res.take::<Vec<PageRow>>(1)?.into_iter().next())
}

/// Delete a page, scrubbing its cross-type knowledge links first.
pub async fn delete_page(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    if get_page(db, owner, uuid).await?.is_none() {
        return Ok(false);
    }
    crate::knowledge::repo::scrub_item_links(db, owner, "page", uuid).await?;
    db.query("DELETE page WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(true)
}
