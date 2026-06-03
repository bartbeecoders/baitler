//! SurrealDB persistence for files and folders. Every query is scoped by
//! `owner`, and timestamps are projected to strings so DTOs serialize cleanly.

use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};

use super::model::{FileRow, FolderRow};

const FOLDER_SELECT: &str = "SELECT uuid, owner, name, parent_id, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM folder";

const FILE_SELECT: &str = "SELECT uuid, owner, name, mime, size, folder_id, storage_key, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM file";

fn vanished() -> AppError {
    AppError::Internal("record not found immediately after write".into())
}

// ── Folders ──────────────────────────────────────────────────────────────────

pub async fn create_folder(
    db: &Db,
    owner: &str,
    name: &str,
    parent_id: Option<&str>,
) -> AppResult<FolderRow> {
    let uuid = Uuid::new_v4().to_string();
    let parent_expr = if parent_id.is_some() {
        "$parent"
    } else {
        "NONE"
    };
    let sql = format!(
        "CREATE folder CONTENT {{ uuid: $uuid, owner: $owner, name: $name, parent_id: {parent_expr} }}; \
         {FOLDER_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut query = db
        .query(sql)
        .bind(("uuid", uuid.clone()))
        .bind(("owner", owner.to_string()))
        .bind(("name", name.to_string()));
    if let Some(p) = parent_id {
        query = query.bind(("parent", p.to_string()));
    }
    let mut res = query.await?.check()?;
    let rows: Vec<FolderRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_folder(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<FolderRow>> {
    let sql = format!("{FOLDER_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    let rows: Vec<FolderRow> = res.take(0)?;
    Ok(rows.into_iter().next())
}

pub async fn list_folders(
    db: &Db,
    owner: &str,
    parent_id: Option<&str>,
) -> AppResult<Vec<FolderRow>> {
    let (clause, parent) = match parent_id {
        Some(p) => ("parent_id = $parent", Some(p.to_string())),
        None => ("parent_id = NONE", None),
    };
    let sql = format!("{FOLDER_SELECT} WHERE owner = $owner AND {clause} ORDER BY name ASC");
    let mut query = db.query(sql).bind(("owner", owner.to_string()));
    if let Some(p) = parent {
        query = query.bind(("parent", p));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

pub async fn update_folder(
    db: &Db,
    owner: &str,
    uuid: &str,
    name: Option<&str>,
    parent_id: Option<Option<&str>>,
) -> AppResult<Option<FolderRow>> {
    let mut sets: Vec<&str> = Vec::new();
    if name.is_some() {
        sets.push("name = $name");
    }
    match parent_id {
        Some(Some(_)) => sets.push("parent_id = $parent"),
        Some(None) => sets.push("parent_id = NONE"),
        None => {}
    }
    if sets.is_empty() {
        return get_folder(db, owner, uuid).await;
    }

    let sql = format!(
        "UPDATE folder SET {} WHERE owner = $owner AND uuid = $uuid; \
         {FOLDER_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(n) = name {
        query = query.bind(("name", n.to_string()));
    }
    if let Some(Some(p)) = parent_id {
        query = query.bind(("parent", p.to_string()));
    }
    let mut res = query.await?.check()?;
    let rows: Vec<FolderRow> = res.take(1)?;
    Ok(rows.into_iter().next())
}

/// Whether a folder contains any sub-folders or files.
pub async fn folder_is_empty(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    if !list_folders(db, owner, Some(uuid)).await?.is_empty() {
        return Ok(false);
    }
    if !list_files(db, owner, Some(uuid)).await?.is_empty() {
        return Ok(false);
    }
    // Pages (Phase 12) reuse this same folder tree via `page.folder_id`, so a
    // folder holding a page is not empty either.
    let mut res = db
        .query("SELECT VALUE uuid FROM page WHERE owner = $owner AND folder_id = $folder")
        .bind(("owner", owner.to_string()))
        .bind(("folder", uuid.to_string()))
        .await?
        .check()?;
    let pages: Vec<String> = res.take(0)?;
    Ok(pages.is_empty())
}

/// Delete a folder by uuid. Callers must ensure it exists and is empty first.
pub async fn delete_folder(db: &Db, owner: &str, uuid: &str) -> AppResult<()> {
    db.query("DELETE folder WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(())
}

// ── Files ────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn create_file(
    db: &Db,
    owner: &str,
    uuid: &str,
    name: &str,
    mime: &str,
    size: i64,
    folder_id: Option<&str>,
    storage_key: &str,
) -> AppResult<FileRow> {
    let folder_expr = if folder_id.is_some() {
        "$folder"
    } else {
        "NONE"
    };
    let sql = format!(
        "CREATE file CONTENT {{ uuid: $uuid, owner: $owner, name: $name, mime: $mime, \
         size: $size, folder_id: {folder_expr}, storage_key: $storage_key }}; \
         {FILE_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut query = db
        .query(sql)
        .bind(("uuid", uuid.to_string()))
        .bind(("owner", owner.to_string()))
        .bind(("name", name.to_string()))
        .bind(("mime", mime.to_string()))
        .bind(("size", size))
        .bind(("storage_key", storage_key.to_string()));
    if let Some(f) = folder_id {
        query = query.bind(("folder", f.to_string()));
    }
    let mut res = query.await?.check()?;
    let rows: Vec<FileRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_file(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<FileRow>> {
    let sql = format!("{FILE_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    let rows: Vec<FileRow> = res.take(0)?;
    Ok(rows.into_iter().next())
}

pub async fn list_files(db: &Db, owner: &str, folder_id: Option<&str>) -> AppResult<Vec<FileRow>> {
    let (clause, folder) = match folder_id {
        Some(f) => ("folder_id = $folder", Some(f.to_string())),
        None => ("folder_id = NONE", None),
    };
    let sql = format!("{FILE_SELECT} WHERE owner = $owner AND {clause} ORDER BY name ASC");
    let mut query = db.query(sql).bind(("owner", owner.to_string()));
    if let Some(f) = folder {
        query = query.bind(("folder", f));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

pub async fn search_files(
    db: &Db,
    owner: &str,
    q: &str,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<FileRow>> {
    let sql = format!(
        "{FILE_SELECT} WHERE owner = $owner \
         AND string::lowercase(name) CONTAINS string::lowercase($q) \
         ORDER BY name ASC LIMIT $limit START $offset"
    );
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("q", q.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

pub async fn update_file(
    db: &Db,
    owner: &str,
    uuid: &str,
    name: Option<&str>,
    folder_id: Option<Option<&str>>,
) -> AppResult<Option<FileRow>> {
    let mut sets: Vec<&str> = Vec::new();
    if name.is_some() {
        sets.push("name = $name");
    }
    match folder_id {
        Some(Some(_)) => sets.push("folder_id = $folder"),
        Some(None) => sets.push("folder_id = NONE"),
        None => {}
    }
    if sets.is_empty() {
        return get_file(db, owner, uuid).await;
    }

    let sql = format!(
        "UPDATE file SET {} WHERE owner = $owner AND uuid = $uuid; \
         {FILE_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(n) = name {
        query = query.bind(("name", n.to_string()));
    }
    if let Some(Some(f)) = folder_id {
        query = query.bind(("folder", f.to_string()));
    }
    let mut res = query.await?.check()?;
    let rows: Vec<FileRow> = res.take(1)?;
    Ok(rows.into_iter().next())
}

/// Delete a file row, returning it (for storage cleanup) if it existed.
pub async fn delete_file(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<FileRow>> {
    // Project the row before deleting so the handler can clean up storage.
    let existing = get_file(db, owner, uuid).await?;
    if existing.is_some() {
        // Scrub cross-type knowledge links (Phase 11) pointing at this file.
        crate::knowledge::repo::scrub_item_links(db, owner, "file", uuid).await?;
        db.query("DELETE file WHERE owner = $owner AND uuid = $uuid")
            .bind(("owner", owner.to_string()))
            .bind(("uuid", uuid.to_string()))
            .await?
            .check()?;
    }
    Ok(existing)
}

/// Path from the root to (and including) `folder_uuid`, root first.
pub async fn breadcrumbs(db: &Db, owner: &str, folder_uuid: &str) -> AppResult<Vec<FolderRow>> {
    let mut chain = Vec::new();
    let mut current = Some(folder_uuid.to_string());
    let mut guard = 0;
    while let Some(uuid) = current {
        guard += 1;
        if guard > 64 {
            break; // defensive against an unexpected cycle
        }
        match get_folder(db, owner, &uuid).await? {
            Some(folder) => {
                current = folder.parent_id.clone();
                chain.push(folder);
            }
            None => break,
        }
    }
    chain.reverse();
    Ok(chain)
}
