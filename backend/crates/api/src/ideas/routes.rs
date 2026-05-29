//! HTTP handlers for ideas.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::model::{
    CreateIdeaBody, IdeaDetailDto, IdeaDto, IdeaSummary, LinkBody, UpdateIdeaBody, STATUSES,
};
use super::repo;

const MAX_LIMIT: usize = 500;
const DEFAULT_LIMIT: usize = 100;
const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 1_000_000;
const MAX_TAGS: usize = 50;
const MAX_TAG_LEN: usize = 50;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ideas", get(list).post(create))
        .route("/ideas/tags", get(tags))
        .route("/ideas/{id}", get(get_one).patch(update).delete(delete))
        .route("/ideas/{id}/links", post(link))
        .route("/ideas/{id}/links/{target}", axum::routing::delete(unlink))
}

#[derive(Debug, Deserialize)]
struct ListParams {
    status: Option<String>,
    tag: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Serialize)]
struct IdeaListResponse {
    ideas: Vec<IdeaDto>,
}

#[derive(Debug, Serialize)]
struct TagsResponse {
    tags: Vec<String>,
}

async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(params): Query<ListParams>,
) -> AppResult<Json<IdeaListResponse>> {
    let status = match params
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) => Some(clean_status(s)?),
        None => None,
    };
    let tag = params
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let q = params.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = params.offset.unwrap_or(0);

    let ideas =
        repo::list_ideas(&state.db, &owner, status.as_deref(), tag, q, limit, offset).await?;
    Ok(Json(IdeaListResponse {
        ideas: ideas.into_iter().map(Into::into).collect(),
    }))
}

async fn tags(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
) -> AppResult<Json<TagsResponse>> {
    let tags = repo::distinct_tags(&state.db, &owner).await?;
    Ok(Json(TagsResponse { tags }))
}

async fn create(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreateIdeaBody>,
) -> AppResult<(StatusCode, Json<IdeaDto>)> {
    let title = clean_title(&body.title)?;
    let text = clean_body(body.body.as_deref().unwrap_or(""))?;
    let tags = clean_tags(body.tags.unwrap_or_default())?;
    let status = match body.status.as_deref() {
        Some(s) => clean_status(s)?,
        None => "inbox".to_string(),
    };
    let idea = repo::create_idea(&state.db, &owner, &title, &text, &tags, &status).await?;
    Ok((StatusCode::CREATED, Json(idea.into())))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<IdeaDetailDto>> {
    let idea = repo::get_idea(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let related = repo::resolve_many(&state.db, &owner, &idea.links).await?;
    Ok(Json(IdeaDetailDto {
        related: related.into_iter().map(IdeaSummary::from).collect(),
        idea: idea.into(),
    }))
}

async fn update(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<UpdateIdeaBody>,
) -> AppResult<Json<IdeaDto>> {
    let db = &state.db;
    repo::get_idea(db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let title = match &body.title {
        Some(t) => Some(clean_title(t)?),
        None => None,
    };
    let text = match &body.body {
        Some(b) => Some(clean_body(b)?),
        None => None,
    };
    let tags = match body.tags {
        Some(t) => Some(clean_tags(t)?),
        None => None,
    };
    let status = match &body.status {
        Some(s) => Some(clean_status(s)?),
        None => None,
    };

    let updated = repo::update_idea(
        db,
        &owner,
        &id,
        title.as_deref(),
        text.as_deref(),
        tags.as_deref(),
        status.as_deref(),
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
    if repo::delete_idea(&state.db, &owner, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

async fn link(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<LinkBody>,
) -> AppResult<StatusCode> {
    if body.target_id == id {
        return Err(AppError::BadRequest("an idea cannot link to itself".into()));
    }
    repo::link_ideas(&state.db, &owner, &id, &body.target_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn unlink(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path((id, target)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    repo::get_idea(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    repo::unlink_ideas(&state.db, &owner, &id, &target).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Validation helpers ───────────────────────────────────────────────────────

fn clean_title(title: &str) -> AppResult<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }
    if trimmed.chars().count() > MAX_TITLE {
        return Err(AppError::BadRequest("title is too long".into()));
    }
    Ok(trimmed.to_string())
}

fn clean_body(body: &str) -> AppResult<String> {
    if body.len() > MAX_BODY {
        return Err(AppError::BadRequest("body is too large".into()));
    }
    Ok(body.to_string())
}

fn clean_status(status: &str) -> AppResult<String> {
    if STATUSES.contains(&status) {
        Ok(status.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "invalid status (expected one of: {})",
            STATUSES.join(", ")
        )))
    }
}

fn clean_tags(tags: Vec<String>) -> AppResult<Vec<String>> {
    let mut cleaned: Vec<String> = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_TAG_LEN {
            return Err(AppError::BadRequest("a tag is too long".into()));
        }
        let tag = trimmed.to_string();
        if !cleaned.contains(&tag) {
            cleaned.push(tag);
        }
    }
    if cleaned.len() > MAX_TAGS {
        return Err(AppError::BadRequest("too many tags".into()));
    }
    Ok(cleaned)
}
