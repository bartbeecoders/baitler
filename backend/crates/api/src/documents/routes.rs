//! HTTP handlers for documents and the shared export pathway.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::convert::{self, SourceFormat, TargetFormat};
use crate::error::{AppError, AppResult};
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::model::{
    CreateDocumentBody, DocumentDto, DocumentSummary, ExportBody, UpdateDocumentBody,
};
use super::repo;

const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 5_000_000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/documents", get(list).post(create))
        .route("/documents/{id}", get(get_one).patch(update).delete(delete))
        .route("/documents/{id}/export", get(export_document))
        .route("/export", post(export_content))
}

#[derive(Debug, Serialize)]
struct DocumentListResponse {
    documents: Vec<DocumentSummary>,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default)]
    tag: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<DocumentListResponse>> {
    let docs = repo::list_documents(
        &state.db,
        &owner,
        q.tag.as_deref().filter(|s| !s.is_empty()),
    )
    .await?;
    Ok(Json(DocumentListResponse {
        documents: docs.into_iter().map(Into::into).collect(),
    }))
}

async fn create(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreateDocumentBody>,
) -> AppResult<(StatusCode, Json<DocumentDto>)> {
    let title = clean_title(&body.title)?;
    let html = clean_body(body.body.as_deref().unwrap_or(""))?;
    let tags = crate::tags::normalize_tags(body.tags.unwrap_or_default())?;
    let doc =
        repo::create_document(&state.db, &owner, &title, &html, "published", None, &tags).await?;
    Ok((StatusCode::CREATED, Json(doc.into())))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<DocumentDto>> {
    let doc = repo::get_document(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(doc.into()))
}

async fn update(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<UpdateDocumentBody>,
) -> AppResult<Json<DocumentDto>> {
    let db = &state.db;
    repo::get_document(db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let title = match &body.title {
        Some(t) => Some(clean_title(t)?),
        None => None,
    };
    let html = match &body.body {
        Some(b) => Some(clean_body(b)?),
        None => None,
    };
    let review = match &body.review {
        Some(r) => Some(clean_review(r)?),
        None => None,
    };
    let tags = match body.tags {
        Some(t) => Some(crate::tags::normalize_tags(t)?),
        None => None,
    };

    let updated = repo::update_document(
        db,
        &owner,
        &id,
        title.as_deref(),
        html.as_deref(),
        review.as_deref(),
        tags.as_deref(),
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
    if repo::delete_document(&state.db, &owner, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

#[derive(Debug, Deserialize)]
struct ExportQuery {
    format: String,
}

/// `GET /documents/:id/export?format=…` — export a stored document.
async fn export_document(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Query(q): Query<ExportQuery>,
) -> AppResult<(HeaderMap, Vec<u8>)> {
    let target = TargetFormat::parse(&q.format)
        .ok_or_else(|| AppError::BadRequest("unsupported export format".into()))?;
    let doc = repo::get_document(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let bytes = convert::export(&doc.body, SourceFormat::Html, target).await?;
    Ok(export_response(bytes, target, &doc.title))
}

/// `POST /export` — the shared conversion pathway for arbitrary content
/// (documents pass HTML, ideas pass Markdown, …).
async fn export_content(
    State(_state): State<AppState>,
    CurrentOwner(_owner): CurrentOwner,
    Json(body): Json<ExportBody>,
) -> AppResult<(HeaderMap, Vec<u8>)> {
    if body.content.len() > MAX_BODY {
        return Err(AppError::BadRequest("content is too large".into()));
    }
    let source = SourceFormat::parse(&body.source)
        .ok_or_else(|| AppError::BadRequest("unsupported source format".into()))?;
    let target = TargetFormat::parse(&body.target)
        .ok_or_else(|| AppError::BadRequest("unsupported target format".into()))?;

    let bytes = convert::export(&body.content, source, target).await?;
    let name = body.filename.as_deref().unwrap_or("export");
    Ok(export_response(bytes, target, name))
}

fn export_response(bytes: Vec<u8>, target: TargetFormat, name: &str) -> (HeaderMap, Vec<u8>) {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(target.content_type()),
    );
    let filename = format!("{}.{}", sanitize_filename(name), target.extension());
    if let Ok(disp) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, disp);
    }
    (headers, bytes)
}

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

/// Sanitize HTML and enforce a size cap.
fn clean_body(body: &str) -> AppResult<String> {
    if body.len() > MAX_BODY {
        return Err(AppError::BadRequest("document is too large".into()));
    }
    Ok(convert::sanitize(body))
}

fn clean_review(review: &str) -> AppResult<String> {
    use crate::ideas::model::REVIEWS;
    if REVIEWS.contains(&review) {
        Ok(review.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "invalid review (expected one of: {})",
            REVIEWS.join(", ")
        )))
    }
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "export".to_string()
    } else {
        trimmed.to_string()
    }
}
