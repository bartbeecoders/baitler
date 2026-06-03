//! HTTP handlers for the knowledge layer: projects, membership, cross-type
//! search, the review queue, and the activity timeline. The portal consumes
//! these; the same repos back the MCP tools, so the two surfaces never drift.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::activity;
use crate::error::{AppError, AppResult};
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::model::{
    MemberCounts, ProjectDto, ProjectMembers, ReviewQueue, SearchResults, TagCount,
    PROJECT_STATUSES,
};
use super::repo;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;
const MAX_NAME: usize = 200;
const MAX_SUMMARY: usize = 1_000_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list).post(create))
        .route("/projects/{id}", get(get_one).patch(update).delete(delete))
        .route("/projects/{id}/items", post(add_item))
        .route(
            "/projects/{id}/items/{item_type}/{item_id}",
            axum::routing::delete(remove_item),
        )
        .route("/knowledge/search", get(search))
        .route("/review", get(review_queue))
        .route("/activity", get(activity_timeline))
        .route("/tags", get(tags))
}

#[derive(Debug, Serialize)]
struct TagsResponse {
    tags: Vec<TagCount>,
}

/// `GET /tags` — the cross-type tag taxonomy (idea + document + page).
async fn tags(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
) -> AppResult<Json<TagsResponse>> {
    Ok(Json(TagsResponse {
        tags: repo::tag_counts(&state.db, &owner).await?,
    }))
}

#[derive(Debug, Serialize)]
struct ProjectListResponse {
    projects: Vec<ProjectDto>,
}

#[derive(Debug, Deserialize)]
struct CreateProjectBody {
    name: String,
    #[serde(default)]
    summary: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct UpdateProjectBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProjectDetail {
    #[serde(flatten)]
    project: ProjectDto,
    counts: MemberCounts,
    members: ProjectMembers,
}

#[derive(Debug, Deserialize)]
struct AddItemBody {
    item_type: String,
    item_id: String,
}

async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
) -> AppResult<Json<ProjectListResponse>> {
    let projects = repo::list_projects(&state.db, &owner).await?;
    Ok(Json(ProjectListResponse {
        projects: projects.into_iter().map(Into::into).collect(),
    }))
}

async fn create(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreateProjectBody>,
) -> AppResult<(StatusCode, Json<ProjectDto>)> {
    let name = clean_name(&body.name)?;
    let summary = clean_summary(body.summary.as_deref().unwrap_or(""))?;
    let project = repo::create_project(&state.db, &owner, &name, &summary).await?;
    Ok((StatusCode::CREATED, Json(project.into())))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<ProjectDetail>> {
    let project = repo::get_project(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let counts = repo::member_counts(&state.db, &owner, &id).await?;
    let members = repo::project_members(&state.db, &owner, &id).await?;
    Ok(Json(ProjectDetail {
        project: project.into(),
        counts,
        members,
    }))
}

async fn update(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<UpdateProjectBody>,
) -> AppResult<Json<ProjectDto>> {
    let name = match &body.name {
        Some(n) => Some(clean_name(n)?),
        None => None,
    };
    let summary = match &body.summary {
        Some(s) => Some(clean_summary(s)?),
        None => None,
    };
    let status = match &body.status {
        Some(s) if PROJECT_STATUSES.contains(&s.as_str()) => Some(s.clone()),
        Some(_) => {
            return Err(AppError::BadRequest(
                "invalid status (expected one of: active, archived)".into(),
            ))
        }
        None => None,
    };
    let updated = repo::update_project(
        &state.db,
        &owner,
        &id,
        name.as_deref(),
        summary.as_deref(),
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
    if repo::delete_project(&state.db, &owner, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

async fn add_item(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<AddItemBody>,
) -> AppResult<StatusCode> {
    repo::get_project(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    repo::set_membership(&state.db, &owner, &body.item_type, &body.item_id, Some(&id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_item(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path((_id, item_type, item_id)): Path<(String, String, String)>,
) -> AppResult<StatusCode> {
    repo::set_membership(&state.db, &owner, &item_type, &item_id, None).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    limit: Option<usize>,
}

async fn search(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(params): Query<SearchQuery>,
) -> AppResult<Json<SearchResults>> {
    let q = params
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest("missing search query `q`".into()))?;
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let results = repo::search(&state.db, &owner, q, limit).await?;
    Ok(Json(results))
}

async fn review_queue(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
) -> AppResult<Json<ReviewQueue>> {
    Ok(Json(repo::review_queue(&state.db, &owner).await?))
}

#[derive(Debug, Deserialize)]
struct ActivityQuery {
    project_id: Option<String>,
    agent: Option<String>,
    since: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ActivityResponse {
    activity: Vec<activity::ActivityDto>,
}

async fn activity_timeline(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(params): Query<ActivityQuery>,
) -> AppResult<Json<ActivityResponse>> {
    let nz = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let project_id = nz(&params.project_id);
    let agent = nz(&params.agent);
    let since = nz(&params.since);
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let rows = activity::list(
        &state.db,
        &owner,
        project_id.as_deref(),
        agent.as_deref(),
        since.as_deref(),
        limit,
    )
    .await?;
    Ok(Json(ActivityResponse {
        activity: rows.into_iter().map(Into::into).collect(),
    }))
}

// ── Validation ────────────────────────────────────────────────────────────────

fn clean_name(name: &str) -> AppResult<String> {
    let t = name.trim();
    if t.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if t.chars().count() > MAX_NAME {
        return Err(AppError::BadRequest("name is too long".into()));
    }
    Ok(t.to_string())
}

fn clean_summary(summary: &str) -> AppResult<String> {
    if summary.len() > MAX_SUMMARY {
        return Err(AppError::BadRequest("summary is too large".into()));
    }
    Ok(summary.to_string())
}
