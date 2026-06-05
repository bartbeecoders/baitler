//! Owner-scoped REST for superpages.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::files::repo as files_repo;
use crate::knowledge::repo as kn_repo;
use crate::owner::CurrentOwner;
use crate::state::AppState;
use crate::tags;

use super::context;
use super::model::{Layout, SuperpageDto, SuperpageSummary};
use super::repo::{self, SuperpagePatch};

const MAX_LIMIT: usize = 500;
const DEFAULT_LIMIT: usize = 100;
const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 5_000_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/superpages", get(list).post(create))
        .route("/superpages/from-project", post(from_project))
        .route(
            "/superpages/{id}",
            get(get_one).patch(update).delete(delete),
        )
        .route("/superpages/{id}/context", get(get_context))
}

fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Debug, Serialize)]
struct SuperpageListResponse {
    superpages: Vec<SuperpageSummary>,
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

fn nz(s: &Option<String>) -> Option<String> {
    s.as_ref()
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
}

fn clean_review_filter(r: &str) -> AppResult<String> {
    if crate::ideas::model::REVIEWS.contains(&r) {
        Ok(r.to_string())
    } else {
        Err(AppError::BadRequest(
            "invalid review (expected draft or published)".into(),
        ))
    }
}

async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(p): Query<ListParams>,
) -> AppResult<Json<SuperpageListResponse>> {
    let limit = p.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = p.offset.unwrap_or(0);
    let review = match nz(&p.review) {
        Some(r) => Some(clean_review_filter(&r)?),
        None => None,
    };
    let rows = repo::list_superpages(
        &state.db,
        &owner,
        nz(&p.folder).as_deref(),
        nz(&p.project).as_deref(),
        nz(&p.tag).as_deref(),
        review.as_deref(),
        nz(&p.q).as_deref(),
        limit,
        offset,
    )
    .await?;
    Ok(Json(SuperpageListResponse {
        superpages: rows.into_iter().map(SuperpageSummary::from_row).collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    title: String,
    #[serde(default)]
    blocks: Option<Value>,
    #[serde(default)]
    folder_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    review: Option<String>,
}

fn parse_layout_value(v: Option<Value>) -> AppResult<Layout> {
    match v {
        Some(val) => {
            let raw = serde_json::to_string(&val)
                .map_err(|e| AppError::BadRequest(format!("invalid blocks: {e}")))?;
            if raw.len() > MAX_BODY {
                return Err(AppError::BadRequest("blocks payload is too large".into()));
            }
            serde_json::from_str(&raw)
                .map_err(|e| AppError::BadRequest(format!("invalid blocks: {e}")))
        }
        None => Ok(Layout::default()),
    }
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

fn clean_review(raw: Option<String>, default_draft: bool) -> AppResult<String> {
    match raw {
        Some(r) if crate::ideas::model::REVIEWS.contains(&r.as_str()) => Ok(r),
        Some(_) => Err(AppError::BadRequest(
            "invalid review (expected draft or published)".into(),
        )),
        None if default_draft => Ok("draft".to_string()),
        None => Ok("published".to_string()),
    }
}

async fn validate_folder(db: &crate::db::Db, owner: &str, folder: Option<&str>) -> AppResult<()> {
    if let Some(id) = folder {
        files_repo::get_folder(db, owner, id)
            .await?
            .ok_or_else(|| AppError::BadRequest("folder does not exist".into()))?;
    }
    Ok(())
}

async fn validate_project(db: &crate::db::Db, owner: &str, project: Option<&str>) -> AppResult<()> {
    if let Some(id) = project {
        kn_repo::get_project(db, owner, id)
            .await?
            .ok_or_else(|| AppError::BadRequest("project does not exist".into()))?;
    }
    Ok(())
}

async fn create(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreateBody>,
) -> AppResult<(StatusCode, Json<SuperpageDto>)> {
    let title = clean_title(&body.title)?;
    let layout = parse_layout_value(body.blocks)?;
    let folder_id = nz(&body.folder_id);
    let project_id = nz(&body.project_id);
    validate_folder(&state.db, &owner, folder_id.as_deref()).await?;
    validate_project(&state.db, &owner, project_id.as_deref()).await?;
    let tags = tags::normalize_tags(body.tags.unwrap_or_default())?;
    let review = clean_review(body.review, true)?;
    let row = repo::create_superpage(
        &state.db,
        &owner,
        &title,
        &layout,
        folder_id.as_deref(),
        project_id.as_deref(),
        &tags,
        &review,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

#[derive(Debug, Deserialize)]
struct FromProjectBody {
    project_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    review: Option<String>,
}

async fn from_project(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<FromProjectBody>,
) -> AppResult<(StatusCode, Json<SuperpageDto>)> {
    let project_id = body.project_id.trim();
    if project_id.is_empty() {
        return Err(AppError::BadRequest("project_id must not be empty".into()));
    }
    let project = kn_repo::get_project(&state.db, &owner, project_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let layout = repo::seed_from_project(&state.db, &owner, project_id).await?;
    let title = body
        .title
        .as_deref()
        .map(clean_title)
        .transpose()?
        .unwrap_or_else(|| format!("{} — board", project.name));
    let review = clean_review(body.review, true)?;
    let row = repo::create_superpage(
        &state.db,
        &owner,
        &title,
        &layout,
        None,
        Some(project_id),
        &[],
        &review,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<SuperpageDto>> {
    let row = repo::get_superpage(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row.into()))
}

async fn get_context(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<context::SuperpageContext>> {
    let row = repo::get_superpage(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let layout: Layout = serde_json::from_str(&row.blocks).unwrap_or_default();
    let ctx = context::resolve_context(&state.db, &owner, &row.uuid, &row.title, &layout).await?;
    Ok(Json(ctx))
}

#[derive(Debug, Default, Deserialize)]
struct UpdateBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    blocks: Option<Value>,
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
    Json(body): Json<UpdateBody>,
) -> AppResult<Json<SuperpageDto>> {
    if let Some(ref f) = body.folder_id {
        if let Some(fid) = f.as_deref() {
            validate_folder(&state.db, &owner, Some(fid)).await?;
        }
    }
    if let Some(ref p) = body.project_id {
        if let Some(pid) = p.as_deref() {
            validate_project(&state.db, &owner, Some(pid)).await?;
        }
    }
    let title = body.title.as_deref().map(clean_title).transpose()?;
    let layout = match body.blocks {
        Some(v) => Some(parse_layout_value(Some(v))?),
        None => None,
    };
    let review = body
        .review
        .map(|r| clean_review(Some(r), false))
        .transpose()?;
    let tags = body.tags.map(tags::normalize_tags).transpose()?;
    let folder_arg = body.folder_id.as_ref().map(|inner| inner.as_deref());
    let project_arg = body.project_id.as_ref().map(|inner| inner.as_deref());

    let updated = repo::update_superpage(
        &state.db,
        &owner,
        &id,
        SuperpagePatch {
            title: title.as_deref(),
            layout: layout.as_ref(),
            review: review.as_deref(),
            folder_id: folder_arg,
            project_id: project_arg,
            tags: tags.as_deref(),
        },
    )
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

async fn delete(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if !repo::delete_superpage(&state.db, &owner, &id).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::superpage::model::BLOCK_KINDS;

    #[test]
    fn block_kinds_are_stable() {
        // typed parts plus retained legacy kinds
        for k in [
            "text", "code", "image", "file", "webpage", "mindmap", "diagram", "embed",
        ] {
            assert!(BLOCK_KINDS.contains(&k), "missing kind {k}");
        }
    }
}
