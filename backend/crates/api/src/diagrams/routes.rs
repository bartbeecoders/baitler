//! Owner-scoped REST for draw.io diagrams. Merged into the auth-gated router
//! tree (behind `CurrentOwner`), alongside `/documents`, `/pages`, `/mindmaps`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::files::repo as files_repo;
use crate::knowledge::repo as kn_repo;
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::model::{DiagramDto, DiagramSummary, MAX_PREVIEW};
use super::repo::{self, DiagramPatch};

const MAX_LIMIT: usize = 500;
const DEFAULT_LIMIT: usize = 100;
const MAX_TITLE: usize = 200;
/// Cap on the mxGraph XML body accepted on a write.
const MAX_XML: usize = 5_000_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/diagrams", get(list).post(create))
        .route("/diagrams/{id}", get(get_one).patch(update).delete(delete))
}

fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Debug, Serialize)]
struct DiagramListResponse {
    diagrams: Vec<DiagramSummary>,
}

#[derive(Debug, Deserialize)]
struct ListParams {
    folder: Option<String>,
    project: Option<String>,
    tag: Option<String>,
    review: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(p): Query<ListParams>,
) -> AppResult<Json<DiagramListResponse>> {
    let folder = nz(&p.folder);
    let project = nz(&p.project);
    let tag = nz(&p.tag);
    let q = nz(&p.q);
    let review = match nz(&p.review) {
        Some(r) => Some(clean_review_filter(&r)?),
        None => None,
    };
    let limit = p.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = p.offset.unwrap_or(0);

    let rows = repo::list_diagrams(
        &state.db,
        &owner,
        folder.as_deref(),
        project.as_deref(),
        tag.as_deref(),
        review.as_deref(),
        q.as_deref(),
        limit,
        offset,
    )
    .await?;
    Ok(Json(DiagramListResponse {
        diagrams: rows.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct CreateDiagramBody {
    title: String,
    #[serde(default)]
    xml: Option<String>,
    #[serde(default)]
    preview: Option<String>,
    #[serde(default)]
    review: Option<String>,
    #[serde(default)]
    folder_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

async fn create(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreateDiagramBody>,
) -> AppResult<(StatusCode, Json<DiagramDto>)> {
    let db = &state.db;
    let title = clean_title(&body.title)?;
    let xml = clean_xml(body.xml.unwrap_or_default())?;
    let preview = clean_preview(body.preview.unwrap_or_default())?;
    let folder_id = validate_folder(db, &owner, body.folder_id.as_deref()).await?;
    let project_id = validate_project(db, &owner, body.project_id.as_deref()).await?;
    let tags = crate::tags::normalize_tags(body.tags.unwrap_or_default())?;
    let review = clean_review(body.review.as_deref())?;

    let row = repo::create_diagram(
        db,
        &owner,
        &title,
        &xml,
        &preview,
        folder_id.as_deref(),
        project_id.as_deref(),
        &tags,
        &review,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<DiagramDto>> {
    let row = repo::get_diagram(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row.into()))
}

#[derive(Debug, Default, Deserialize)]
struct UpdateDiagramBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    xml: Option<String>,
    #[serde(default)]
    preview: Option<String>,
    #[serde(default)]
    review: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    folder_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    project_id: Option<Option<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

async fn update(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<UpdateDiagramBody>,
) -> AppResult<Json<DiagramDto>> {
    let db = &state.db;
    let title = match &body.title {
        Some(t) => Some(clean_title(t)?),
        None => None,
    };
    let xml = match body.xml {
        Some(x) => Some(clean_xml(x)?),
        None => None,
    };
    let preview = match body.preview {
        Some(p) => Some(clean_preview(p)?),
        None => None,
    };
    let review = match &body.review {
        Some(r) => Some(clean_review_filter(r)?),
        None => None,
    };
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
    let tags = match body.tags {
        Some(t) => Some(crate::tags::normalize_tags(t)?),
        None => None,
    };

    let patch = DiagramPatch {
        title: title.as_deref(),
        xml: xml.as_deref(),
        preview: preview.as_deref(),
        review: review.as_deref(),
        folder_id: body.folder_id.as_ref().map(|o| o.as_deref()),
        project_id: body.project_id.as_ref().map(|o| o.as_deref()),
        tags: tags.as_deref(),
    };
    let updated = repo::update_diagram(db, &owner, &id, patch)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

async fn delete(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if repo::delete_diagram(&state.db, &owner, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
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

fn clean_xml(xml: String) -> AppResult<String> {
    if xml.len() > MAX_XML {
        return Err(AppError::BadRequest("diagram is too large".into()));
    }
    Ok(xml)
}

/// A preview must be a `data:` URI (rendered in an `<img>`, never executed) and
/// within the size cap. An empty preview is allowed (no thumbnail yet).
fn clean_preview(preview: String) -> AppResult<String> {
    if preview.is_empty() {
        return Ok(preview);
    }
    if preview.len() > MAX_PREVIEW {
        return Err(AppError::BadRequest("preview is too large".into()));
    }
    if !preview.starts_with("data:image/") {
        return Err(AppError::BadRequest(
            "preview must be a data:image/* URI".into(),
        ));
    }
    Ok(preview)
}

fn clean_review(review: Option<&str>) -> AppResult<String> {
    match review {
        Some(r) if crate::ideas::model::REVIEWS.contains(&r) => Ok(r.to_string()),
        Some(_) => Err(AppError::BadRequest(
            "invalid review (expected one of: draft, published)".into(),
        )),
        None => Ok("published".to_string()),
    }
}

fn clean_review_filter(review: &str) -> AppResult<String> {
    if crate::ideas::model::REVIEWS.contains(&review) {
        Ok(review.to_string())
    } else {
        Err(AppError::BadRequest(
            "invalid review (expected one of: draft, published)".into(),
        ))
    }
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
