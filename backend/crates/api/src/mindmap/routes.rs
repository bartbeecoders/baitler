//! Owner-scoped REST for mindmaps. Merged into the auth-gated router tree
//! (behind `CurrentOwner`), alongside `/documents` and `/pages`.

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

use super::model::{
    from_markdown_outline, Graph, MindmapDto, MindmapSummary, MAX_NODES, SOURCE_FORMATS,
};
use super::repo::{self, MindmapPatch};

const MAX_LIMIT: usize = 500;
const DEFAULT_LIMIT: usize = 100;
const MAX_TITLE: usize = 200;
/// Cap on a raw outline / serialized graph body accepted on a write.
const MAX_BODY: usize = 5_000_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mindmaps", get(list).post(create))
        .route("/mindmaps/{id}", get(get_one).patch(update).delete(delete))
        .route("/mindmaps/from-project", post(from_project))
}

fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Debug, Serialize)]
struct MindmapListResponse {
    mindmaps: Vec<MindmapSummary>,
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
) -> AppResult<Json<MindmapListResponse>> {
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

    let rows = repo::list_mindmaps(
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
    Ok(Json(MindmapListResponse {
        mindmaps: rows.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct CreateMindmapBody {
    title: String,
    /// JSON `{ nodes, edges }` graph (canvas authoring).
    #[serde(default)]
    graph: Option<Value>,
    /// Markdown outline to seed the graph from (sets source_format=markdown).
    #[serde(default)]
    outline: Option<String>,
    #[serde(default)]
    source_format: Option<String>,
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
    Json(body): Json<CreateMindmapBody>,
) -> AppResult<(StatusCode, Json<MindmapDto>)> {
    let db = &state.db;
    let title = clean_title(&body.title)?;

    // Outline → markdown source; otherwise a JSON graph (default empty).
    let (graph, source_format) = match body.outline {
        Some(outline) => {
            if outline.len() > MAX_BODY {
                return Err(AppError::BadRequest("outline is too large".into()));
            }
            (from_markdown_outline(&outline), "markdown".to_string())
        }
        None => {
            let fmt = match body.source_format.as_deref() {
                Some(f) => clean_source_format(f)?,
                None => "json".to_string(),
            };
            (parse_graph(body.graph)?, fmt)
        }
    };

    let folder_id = validate_folder(db, &owner, body.folder_id.as_deref()).await?;
    let project_id = validate_project(db, &owner, body.project_id.as_deref()).await?;
    let tags = crate::tags::normalize_tags(body.tags.unwrap_or_default())?;
    // Portal writes default to published (human-authored); the MCP path defaults to draft.
    let review = clean_review(body.review.as_deref())?;

    let row = repo::create_mindmap(
        db,
        &owner,
        &title,
        &graph,
        &source_format,
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
    folder_id: Option<String>,
    #[serde(default)]
    review: Option<String>,
}

/// Seed a new mindmap from a project's ideas + cross-links, persist it, return it.
async fn from_project(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<FromProjectBody>,
) -> AppResult<(StatusCode, Json<MindmapDto>)> {
    let db = &state.db;
    let project = kn_repo::get_project(db, &owner, &body.project_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("project does not exist".into()))?;
    let graph = repo::seed_from_project(db, &owner, &body.project_id).await?;
    let title = match body.title {
        Some(t) => clean_title(&t)?,
        None => clean_title(&format!("{} — mindmap", project.name))?,
    };
    let folder_id = validate_folder(db, &owner, body.folder_id.as_deref()).await?;
    let review = clean_review(body.review.as_deref())?;
    let row = repo::create_mindmap(
        db,
        &owner,
        &title,
        &graph,
        "json",
        folder_id.as_deref(),
        Some(&body.project_id),
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
) -> AppResult<Json<MindmapDto>> {
    let row = repo::get_mindmap(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row.into()))
}

#[derive(Debug, Default, Deserialize)]
struct UpdateMindmapBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    graph: Option<Value>,
    #[serde(default)]
    outline: Option<String>,
    #[serde(default)]
    source_format: Option<String>,
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
    Json(body): Json<UpdateMindmapBody>,
) -> AppResult<Json<MindmapDto>> {
    let db = &state.db;
    let title = match &body.title {
        Some(t) => Some(clean_title(t)?),
        None => None,
    };
    // Graph comes from `outline` (markdown) or `graph` (json); outline wins.
    let mut source_format = match &body.source_format {
        Some(f) => Some(clean_source_format(f)?),
        None => None,
    };
    let graph: Option<Graph> = match &body.outline {
        Some(outline) => {
            if outline.len() > MAX_BODY {
                return Err(AppError::BadRequest("outline is too large".into()));
            }
            source_format = Some("markdown".to_string());
            Some(from_markdown_outline(outline))
        }
        None => match body.graph {
            Some(v) => Some(parse_graph(Some(v))?),
            None => None,
        },
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

    let patch = MindmapPatch {
        title: title.as_deref(),
        graph: graph.as_ref(),
        source_format: source_format.as_deref(),
        review: review.as_deref(),
        folder_id: body.folder_id.as_ref().map(|o| o.as_deref()),
        project_id: body.project_id.as_ref().map(|o| o.as_deref()),
        tags: tags.as_deref(),
    };
    let updated = repo::update_mindmap(db, &owner, &id, patch)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

async fn delete(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if repo::delete_mindmap(&state.db, &owner, &id).await? {
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

/// Parse an optional JSON graph value into a `Graph`, capping serialized size.
fn parse_graph(value: Option<Value>) -> AppResult<Graph> {
    match value {
        Some(v) => {
            let graph: Graph = serde_json::from_value(v)
                .map_err(|e| AppError::BadRequest(format!("invalid graph: {e}")))?;
            if graph.nodes.len() > MAX_NODES {
                return Err(AppError::BadRequest("too many nodes".into()));
            }
            Ok(graph)
        }
        None => Ok(Graph::default()),
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

/// Portal review default is `published`; an explicit valid value is honored.
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
