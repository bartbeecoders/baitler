//! SurrealDB persistence for superpages.

use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::knowledge::repo as kn;

use super::model::{
    derive_search_text, layout_from_member_embeds, normalize_refs, referenced, validate_layout,
    Layout, SuperpageRow,
};

const SUPERPAGE_SELECT: &str = "SELECT uuid, owner, title, blocks, folder_id, \
    project_id, tags, review, version, \
    IF published_at = NONE THEN NONE ELSE type::string(published_at) END AS published_at, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM superpage";

fn vanished() -> AppError {
    AppError::Internal("superpage not found immediately after write".into())
}

/// Collect referenced-item titles for search_text (best-effort).
async fn collect_ref_titles(db: &Db, owner: &str, layout: &Layout) -> AppResult<Vec<String>> {
    let mut titles = Vec::new();
    for block in &layout.blocks {
        if let Some((it, id)) = referenced(block) {
            if let Some(t) = kn::resolve_title(db, owner, it, id).await? {
                titles.push(t);
            }
        }
    }
    Ok(titles)
}

async fn prepare_layout(
    db: &Db,
    owner: &str,
    title: &str,
    layout: &Layout,
) -> AppResult<(String, String)> {
    // Trim referencing ids/types once so the stored, existence-checked, and
    // later-resolved ids are byte-identical.
    let normalized = normalize_refs(layout);
    let layout = &normalized;
    validate_layout(layout).map_err(AppError::BadRequest)?;
    for block in &layout.blocks {
        if let Some((it, id)) = referenced(block) {
            if !kn::item_exists(db, owner, it, id).await? {
                return Err(AppError::BadRequest(format!(
                    "part references a missing {it} `{id}`"
                )));
            }
        }
    }
    let ref_titles: Vec<String> = collect_ref_titles(db, owner, layout).await?;
    let refs: Vec<&str> = ref_titles.iter().map(String::as_str).collect();
    let search_text = derive_search_text(title, layout, &refs);
    let json = serde_json::to_string(layout)
        .map_err(|e| AppError::Internal(format!("blocks serialize failed: {e}").into()))?;
    Ok((json, search_text))
}

#[allow(clippy::too_many_arguments)]
pub async fn create_superpage(
    db: &Db,
    owner: &str,
    title: &str,
    layout: &Layout,
    folder_id: Option<&str>,
    project_id: Option<&str>,
    tags: &[String],
    review: &str,
) -> AppResult<SuperpageRow> {
    let uuid = Uuid::new_v4().to_string();
    let (blocks_json, search_text) = prepare_layout(db, owner, title, layout).await?;
    let sql = format!(
        "CREATE superpage CONTENT {{ uuid: $uuid, owner: $owner, title: $title, blocks: $blocks, \
         search_text: $search_text, folder_id: $folder_id, project_id: $project_id, tags: $tags, \
         review: $review }}; \
         {SUPERPAGE_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("title", title.to_string()))
        .bind(("blocks", blocks_json))
        .bind(("search_text", search_text))
        .bind(("folder_id", folder_id.map(str::to_string)))
        .bind(("project_id", project_id.map(str::to_string)))
        .bind(("tags", tags.to_vec()))
        .bind(("review", review.to_string()))
        .await?
        .check()?;
    let rows: Vec<SuperpageRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_superpage(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<SuperpageRow>> {
    let sql = format!("{SUPERPAGE_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<SuperpageRow>>(0)?.into_iter().next())
}

#[allow(clippy::too_many_arguments)]
pub async fn list_superpages(
    db: &Db,
    owner: &str,
    folder: Option<&str>,
    project: Option<&str>,
    tag: Option<&str>,
    review: Option<&str>,
    q: Option<&str>,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<SuperpageRow>> {
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
        "{SUPERPAGE_SELECT} WHERE {} ORDER BY updated_at DESC LIMIT $limit START $offset",
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

#[derive(Default)]
pub struct SuperpagePatch<'a> {
    pub title: Option<&'a str>,
    pub layout: Option<&'a Layout>,
    pub review: Option<&'a str>,
    pub folder_id: Option<Option<&'a str>>,
    pub project_id: Option<Option<&'a str>>,
    pub tags: Option<&'a [String]>,
}

pub async fn update_superpage(
    db: &Db,
    owner: &str,
    uuid: &str,
    patch: SuperpagePatch<'_>,
) -> AppResult<Option<SuperpageRow>> {
    let Some(current) = get_superpage(db, owner, uuid).await? else {
        return Ok(None);
    };

    let prepared = match patch.layout {
        Some(l) => Some(prepare_layout(db, owner, patch.title.unwrap_or(&current.title), l).await?),
        None => None,
    };

    let mut sets: Vec<&str> = Vec::new();
    if patch.title.is_some() {
        sets.push("title = $title");
    }
    if prepared.is_some() {
        sets.push("blocks = $blocks");
    }
    if patch.title.is_some() || prepared.is_some() {
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
    if patch.title.is_some() || prepared.is_some() {
        sets.push("version = version + 1");
    }
    if sets.is_empty() {
        return Ok(Some(current));
    }

    let new_title = patch.title.unwrap_or(&current.title).to_string();
    let needs_search_text = patch.title.is_some() || prepared.is_some();
    // Only recompute (which costs one resolve_title query per referenced part)
    // when search_text will actually be written.
    let search_text = if needs_search_text {
        match &prepared {
            Some((_, st)) => st.clone(),
            None => {
                let layout: Layout = serde_json::from_str(&current.blocks).unwrap_or_default();
                let ref_titles: Vec<String> = collect_ref_titles(db, owner, &layout).await?;
                let refs: Vec<&str> = ref_titles.iter().map(String::as_str).collect();
                derive_search_text(&new_title, &layout, &refs)
            }
        }
    } else {
        String::new()
    };

    let sql = format!(
        "UPDATE superpage SET {} WHERE owner = $owner AND uuid = $uuid; \
         {SUPERPAGE_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(t) = patch.title {
        query = query.bind(("title", t.to_string()));
    }
    if let Some((json, _)) = &prepared {
        query = query.bind(("blocks", json.clone()));
    }
    if patch.title.is_some() || prepared.is_some() {
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
    Ok(res.take::<Vec<SuperpageRow>>(1)?.into_iter().next())
}

pub async fn delete_superpage(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    if get_superpage(db, owner, uuid).await?.is_none() {
        return Ok(false);
    }
    kn::scrub_item_links(db, owner, "superpage", uuid).await?;
    db.query("DELETE superpage WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(true)
}

/// Build a layout whose embed blocks reference every member of a project.
pub async fn seed_from_project(db: &Db, owner: &str, project_id: &str) -> AppResult<Layout> {
    if kn::get_project(db, owner, project_id).await?.is_none() {
        return Err(AppError::NotFound);
    }
    let members = kn::project_members(db, owner, project_id).await?;
    let ideas: Vec<(String, String)> = members
        .ideas
        .iter()
        .map(|m| (m.id.clone(), m.title.clone()))
        .collect();
    let documents: Vec<(String, String)> = members
        .documents
        .iter()
        .map(|m| (m.id.clone(), m.title.clone()))
        .collect();
    let files: Vec<(String, String)> = members
        .files
        .iter()
        .map(|m| (m.id.clone(), m.title.clone()))
        .collect();
    let pages: Vec<(String, String)> = members
        .pages
        .iter()
        .map(|m| (m.id.clone(), m.title.clone()))
        .collect();
    let mindmaps: Vec<(String, String)> = members
        .mindmaps
        .iter()
        .map(|m| (m.id.clone(), m.title.clone()))
        .collect();
    let diagrams: Vec<(String, String)> = members
        .diagrams
        .iter()
        .map(|m| (m.id.clone(), m.title.clone()))
        .collect();
    Ok(layout_from_member_embeds(
        &ideas, &documents, &files, &pages, &mindmaps, &diagrams,
    ))
}
