//! Owner-scoped REST for hosted pages. Merged into the auth-gated router tree
//! (behind `CurrentOwner`), alongside `/documents`. The unauthenticated public
//! serve route (`GET /p/{slug}`) lives in [`super::public`] (Phase 12.5) and is
//! wired OUTSIDE this layer.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::documents::repo as doc_repo;
use crate::error::{AppError, AppResult};
use crate::files::repo as files_repo;
use crate::knowledge::repo as kn_repo;
use crate::owner::CurrentOwner;
use crate::slug::slugify;
use crate::state::AppState;

use super::model::{PageDto, PageSummary, SOURCE_FORMATS, VISIBILITIES};
use super::repo::{self, PagePatch};

const MAX_LIMIT: usize = 500;
const DEFAULT_LIMIT: usize = 100;
const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 5_000_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/pages", get(list).post(create))
        .route("/pages/{id}", get(get_one).patch(update).delete(delete))
        .route("/pages/{id}/publish", post(publish))
        .route("/pages/{id}/unpublish", post(unpublish))
}

/// Deserialize a present field (incl. explicit `null`) to `Some(inner)`, an
/// absent field to `None` — the move-vs-leave distinction for folder/project.
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Debug, Serialize)]
struct PageListResponse {
    pages: Vec<PageSummary>,
}

#[derive(Debug, Deserialize)]
struct ListParams {
    folder: Option<String>,
    visibility: Option<String>,
    project: Option<String>,
    q: Option<String>,
    /// Accepted for forward-compat; a no-op pre-auth (one owner). See Phase 12.D.
    #[allow(dead_code)]
    author: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(p): Query<ListParams>,
) -> AppResult<Json<PageListResponse>> {
    let folder = nz(&p.folder);
    let project = nz(&p.project);
    let q = nz(&p.q);
    let visibility = match nz(&p.visibility) {
        Some(v) => Some(clean_visibility(&v)?),
        None => None,
    };
    let limit = p.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = p.offset.unwrap_or(0);

    let pages = repo::list_pages(
        &state.db,
        &owner,
        folder.as_deref(),
        visibility.as_deref(),
        project.as_deref(),
        q.as_deref(),
        limit,
        offset,
    )
    .await?;
    Ok(Json(PageListResponse {
        pages: pages.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct CreatePageBody {
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    source_format: Option<String>,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    folder_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    /// Promote an existing document into a new page (copies its sanitized HTML).
    #[serde(default)]
    from_document: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreatePageBody>,
) -> AppResult<(StatusCode, Json<PageDto>)> {
    let db = &state.db;
    let title = clean_title(&body.title)?;
    let visibility = match body.visibility.as_deref() {
        Some(v) => clean_visibility(v)?,
        None => "draft".to_string(),
    };

    // `from_document` is the one-way promote bridge: copy a document's already-
    // sanitized HTML body. Otherwise use the supplied body + source format.
    let (raw_body, source_format) = match body.from_document.as_deref() {
        Some(doc_id) => {
            let doc = doc_repo::get_document(db, &owner, doc_id)
                .await?
                .ok_or_else(|| AppError::BadRequest("from_document does not exist".into()))?;
            (doc.body, "html".to_string())
        }
        None => {
            let fmt = match body.source_format.as_deref() {
                Some(f) => clean_source_format(f)?,
                None => "html".to_string(),
            };
            (body.body.unwrap_or_default(), fmt)
        }
    };
    if raw_body.len() > MAX_BODY {
        return Err(AppError::BadRequest("page is too large".into()));
    }

    let folder_id = validate_folder(db, &owner, body.folder_id.as_deref()).await?;
    let project_id = validate_project(db, &owner, body.project_id.as_deref()).await?;

    let page = repo::create_page(
        db,
        &owner,
        &title,
        &raw_body,
        &source_format,
        &visibility,
        folder_id.as_deref(),
        project_id.as_deref(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(page.into())))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<PageDto>> {
    let page = repo::get_page(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(page.into()))
}

#[derive(Debug, Default, Deserialize)]
struct UpdatePageBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    source_format: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    folder_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    project_id: Option<Option<String>>,
}

async fn update(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<UpdatePageBody>,
) -> AppResult<Json<PageDto>> {
    let db = &state.db;
    let title = match &body.title {
        Some(t) => Some(clean_title(t)?),
        None => None,
    };
    if let Some(b) = &body.body {
        if b.len() > MAX_BODY {
            return Err(AppError::BadRequest("page is too large".into()));
        }
    }
    let source_format = match &body.source_format {
        Some(f) => Some(clean_source_format(f)?),
        None => None,
    };
    let visibility = match &body.visibility {
        Some(v) => Some(clean_visibility(v)?),
        None => None,
    };
    // A custom slug is normalized to the slug charset and re-checked owner-unique.
    let slug = match &body.slug {
        Some(s) => Some(clean_custom_slug(db, &owner, &id, s).await?),
        None => None,
    };
    // Validate move targets exist (Some(Some) = move; Some(None) = clear).
    if let Some(Some(f)) = body.folder_id.as_ref() {
        files_repo::get_folder(db, &owner, f)
            .await?
            .ok_or_else(|| AppError::BadRequest("target folder does not exist".into()))?;
    }
    if let Some(Some(p)) = body.project_id.as_ref() {
        kn_repo::get_project(db, &owner, p)
            .await?
            .ok_or_else(|| AppError::BadRequest("target project does not exist".into()))?;
    }

    let patch = PagePatch {
        title: title.as_deref(),
        body: body.body.as_deref(),
        source_format: source_format.as_deref(),
        slug: slug.as_deref(),
        visibility: visibility.as_deref(),
        folder_id: body.folder_id.as_ref().map(|o| o.as_deref()),
        project_id: body.project_id.as_ref().map(|o| o.as_deref()),
    };
    let updated = repo::update_page(db, &owner, &id, patch)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

async fn delete(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if repo::delete_page(&state.db, &owner, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

#[derive(Debug, Default, Deserialize)]
struct PublishBody {
    /// `unlisted` or `public`; defaults to `public`.
    #[serde(default)]
    visibility: Option<String>,
}

async fn publish(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<PublishBody>,
) -> AppResult<Json<PageDto>> {
    let visibility = match body.visibility.as_deref() {
        Some(v) => clean_visibility(v)?,
        None => "public".to_string(),
    };
    if visibility == "draft" {
        return Err(AppError::BadRequest(
            "publish expects unlisted or public; use /unpublish for draft".into(),
        ));
    }
    let patch = PagePatch {
        visibility: Some(&visibility),
        ..PagePatch::default()
    };
    let updated = repo::update_page(&state.db, &owner, &id, patch)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

async fn unpublish(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<PageDto>> {
    let patch = PagePatch {
        visibility: Some("draft"),
        ..PagePatch::default()
    };
    let updated = repo::update_page(&state.db, &owner, &id, patch)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

// ── Validation helpers ────────────────────────────────────────────────────────

fn nz(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn clean_title(title: &str) -> AppResult<String> {
    let t = title.trim();
    if t.is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }
    if t.chars().count() > MAX_TITLE {
        return Err(AppError::BadRequest("title is too long".into()));
    }
    Ok(t.to_string())
}

fn clean_source_format(fmt: &str) -> AppResult<String> {
    if SOURCE_FORMATS.contains(&fmt) {
        Ok(fmt.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "invalid source_format (expected one of: {})",
            SOURCE_FORMATS.join(", ")
        )))
    }
}

fn clean_visibility(v: &str) -> AppResult<String> {
    if VISIBILITIES.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "invalid visibility (expected one of: {})",
            VISIBILITIES.join(", ")
        )))
    }
}

/// Normalize a user-supplied slug to the slug charset and ensure it is free
/// within the owner's namespace (allowing the page to keep its own slug).
async fn clean_custom_slug(
    db: &crate::db::Db,
    owner: &str,
    page_id: &str,
    raw: &str,
) -> AppResult<String> {
    let slug = slugify(raw, "page");
    if let Some(existing) = repo::get_page_by_slug(db, owner, &slug).await? {
        if existing.uuid != page_id {
            return Err(AppError::Conflict("that slug is already in use".into()));
        }
    }
    Ok(slug)
}

async fn validate_folder(
    db: &crate::db::Db,
    owner: &str,
    folder_id: Option<&str>,
) -> AppResult<Option<String>> {
    match folder_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(f) => {
            files_repo::get_folder(db, owner, f)
                .await?
                .ok_or_else(|| AppError::BadRequest("target folder does not exist".into()))?;
            Ok(Some(f.to_string()))
        }
        None => Ok(None),
    }
}

async fn validate_project(
    db: &crate::db::Db,
    owner: &str,
    project_id: Option<&str>,
) -> AppResult<Option<String>> {
    match project_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => {
            kn_repo::get_project(db, owner, p)
                .await?
                .ok_or_else(|| AppError::BadRequest("target project does not exist".into()))?;
            Ok(Some(p.to_string()))
        }
        None => Ok(None),
    }
}
