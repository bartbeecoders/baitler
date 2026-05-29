//! HTTP handlers for files and folders.

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::model::{
    CreateFolderBody, FileDto, FolderDto, ListingDto, UpdateFileBody, UpdateFolderBody,
};
use super::repo;

const MAX_LIMIT: usize = 500;
const DEFAULT_LIMIT: usize = 100;

/// Map a multipart error to an [`AppError`], preserving a body-size overflow as
/// 413 (the body-limit layer surfaces it as a multipart read error otherwise).
fn multipart_error(e: axum::extract::multipart::MultipartError) -> AppError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::PayloadTooLarge("upload exceeds the maximum allowed size".to_string())
    } else {
        AppError::BadRequest(format!("invalid multipart body: {e}"))
    }
}

/// Build the files/folders router. `max_upload_bytes` bounds request bodies.
pub fn router(max_upload_bytes: usize) -> Router<AppState> {
    Router::new()
        .route("/files", get(list).post(upload))
        .route(
            "/files/{id}",
            get(get_one).patch(update_file).delete(delete_file),
        )
        .route("/files/{id}/content", get(download))
        .route("/folders", post(create_folder))
        .route("/folders/{id}", patch(update_folder).delete(delete_folder))
        .layer(DefaultBodyLimit::max(max_upload_bytes))
}

#[derive(Debug, Deserialize)]
struct ListParams {
    folder: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct UploadParams {
    folder: Option<String>,
}

#[derive(Debug, Serialize)]
struct UploadResponse {
    files: Vec<FileDto>,
}

// ── Listing & search ───────────────────────────────────────────────────────

/// `GET /files` — list a folder's contents, or search files when `q` is given.
async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(params): Query<ListParams>,
) -> AppResult<Json<ListingDto>> {
    let db = &state.db;

    // Search mode: a flat list of matching files across all folders.
    if let Some(q) = params.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
        let offset = params.offset.unwrap_or(0);
        let files = repo::search_files(db, &owner, q, limit, offset).await?;
        return Ok(Json(ListingDto {
            folder: None,
            breadcrumbs: Vec::new(),
            folders: Vec::new(),
            files: files.into_iter().map(Into::into).collect(),
        }));
    }

    let folder_id = params.folder.as_deref();

    // Validate the requested folder exists (and is owned) before listing it.
    let (current, breadcrumbs) = match folder_id {
        Some(id) => {
            let folder = repo::get_folder(db, &owner, id)
                .await?
                .ok_or(AppError::NotFound)?;
            let crumbs = repo::breadcrumbs(db, &owner, id).await?;
            (Some(folder.into()), crumbs)
        }
        None => (None, Vec::new()),
    };

    let folders = repo::list_folders(db, &owner, folder_id).await?;
    let files = repo::list_files(db, &owner, folder_id).await?;

    Ok(Json(ListingDto {
        folder: current,
        breadcrumbs: breadcrumbs.into_iter().map(Into::into).collect(),
        folders: folders.into_iter().map(Into::into).collect(),
        files: files.into_iter().map(Into::into).collect(),
    }))
}

// ── Folders ────────────────────────────────────────────────────────────────

async fn create_folder(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Json(body): Json<CreateFolderBody>,
) -> AppResult<(StatusCode, Json<FolderDto>)> {
    let db = &state.db;
    let name = clean_name(&body.name)?;

    if let Some(parent) = body.parent_id.as_deref() {
        repo::get_folder(db, &owner, parent)
            .await?
            .ok_or_else(|| AppError::BadRequest("parent folder does not exist".into()))?;
    }

    let folder = repo::create_folder(db, &owner, &name, body.parent_id.as_deref()).await?;
    Ok((StatusCode::CREATED, Json(folder.into())))
}

async fn update_folder(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<UpdateFolderBody>,
) -> AppResult<Json<FolderDto>> {
    let db = &state.db;
    repo::get_folder(db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let name = match &body.name {
        Some(n) => Some(clean_name(n)?),
        None => None,
    };

    if let Some(Some(parent)) = body.parent_id.as_ref() {
        repo::get_folder(db, &owner, parent)
            .await?
            .ok_or_else(|| AppError::BadRequest("target folder does not exist".into()))?;
        if parent == &id {
            return Err(AppError::BadRequest(
                "a folder cannot contain itself".into(),
            ));
        }
        let ancestors = repo::breadcrumbs(db, &owner, parent).await?;
        if ancestors.iter().any(|f| f.uuid == id) {
            return Err(AppError::BadRequest(
                "cannot move a folder into one of its descendants".into(),
            ));
        }
    }

    let parent_arg = body.parent_id.as_ref().map(|inner| inner.as_deref());
    let updated = repo::update_folder(db, &owner, &id, name.as_deref(), parent_arg)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

async fn delete_folder(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let db = &state.db;
    repo::get_folder(db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    if !repo::folder_is_empty(db, &owner, &id).await? {
        return Err(AppError::Conflict("folder is not empty".into()));
    }
    repo::delete_folder(db, &owner, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Files ──────────────────────────────────────────────────────────────────

async fn upload(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(params): Query<UploadParams>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<UploadResponse>)> {
    let db = &state.db;

    if let Some(folder) = params.folder.as_deref() {
        repo::get_folder(db, &owner, folder)
            .await?
            .ok_or_else(|| AppError::BadRequest("target folder does not exist".into()))?;
    }

    // Keys created this request, so a mid-batch failure rolls back ALL of them
    // (not just the current file) — the response is all-or-nothing.
    let mut created = Vec::new();
    let mut created_keys: Vec<String> = Vec::new();

    let outcome: AppResult<()> = async {
        while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
            if field.name() != Some("file") {
                continue;
            }
            let name = sanitize_filename(field.file_name().unwrap_or("untitled"));
            let mime = sanitize_mime(field.content_type());
            let bytes = field.bytes().await.map_err(multipart_error)?;

            let storage_key = Uuid::new_v4().to_string();
            state.storage.put(&storage_key, bytes.as_ref()).await?;
            created_keys.push(storage_key.clone());

            let file = repo::create_file(
                db,
                &owner,
                &storage_key, // public uuid == storage key
                &name,
                &mime,
                bytes.len() as i64,
                params.folder.as_deref(),
                &storage_key,
            )
            .await?;
            created.push(FileDto::from(file));
        }
        Ok(())
    }
    .await;

    if let Err(e) = outcome {
        // Best-effort rollback of every object/row created in this request.
        for key in &created_keys {
            if let Err(err) = state.storage.delete(key).await {
                tracing::warn!(error = %err, key, "upload rollback: storage delete failed");
            }
            let _ = repo::delete_file(db, &owner, key).await;
        }
        return Err(e);
    }

    if created.is_empty() {
        return Err(AppError::BadRequest(
            "no file provided (expected a multipart field named \"file\")".into(),
        ));
    }

    Ok((StatusCode::CREATED, Json(UploadResponse { files: created })))
}

async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<FileDto>> {
    let file = repo::get_file(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(file.into()))
}

async fn download(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<(HeaderMap, Body)> {
    let file = repo::get_file(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let reader = state.storage.open(&file.storage_key).await?;
    let body = Body::from_stream(ReaderStream::new(reader));

    // Only a small allowlist of safe types is served inline; everything else is
    // an attachment. Combined with nosniff + a locked-down CSP, this prevents an
    // uploaded text/html or SVG from executing in the app origin.
    let disposition_kind = if is_inline_safe(&file.mime) {
        "inline"
    } else {
        "attachment"
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&file.mime)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    if let Ok(len) = HeaderValue::from_str(&file.size.to_string()) {
        headers.insert(header::CONTENT_LENGTH, len);
    }
    if let Ok(disp) = HeaderValue::from_str(&content_disposition(disposition_kind, &file.name)) {
        headers.insert(header::CONTENT_DISPOSITION, disp);
    }

    Ok((headers, body))
}

async fn update_file(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
    Json(body): Json<UpdateFileBody>,
) -> AppResult<Json<FileDto>> {
    let db = &state.db;
    repo::get_file(db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let name = match &body.name {
        Some(n) => Some(clean_name(n)?),
        None => None,
    };

    if let Some(Some(folder)) = body.folder_id.as_ref() {
        repo::get_folder(db, &owner, folder)
            .await?
            .ok_or_else(|| AppError::BadRequest("target folder does not exist".into()))?;
    }

    let folder_arg = body.folder_id.as_ref().map(|inner| inner.as_deref());
    let updated = repo::update_file(db, &owner, &id, name.as_deref(), folder_arg)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(updated.into()))
}

async fn delete_file(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let deleted = repo::delete_file(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    // Best-effort storage cleanup; the DB record (source of truth) is already gone.
    if let Err(e) = state.storage.delete(&deleted.storage_key).await {
        tracing::warn!(error = %e, key = %deleted.storage_key, "failed to delete storage object");
    }
    Ok(StatusCode::NO_CONTENT)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Validate and normalize a user-supplied folder/file name.
fn clean_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if trimmed.len() > 255 || trimmed.contains(['/', '\\', '\0']) {
        return Err(AppError::BadRequest(
            "name contains invalid characters".into(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Types served inline (rendered) on download. Everything else is an attachment.
/// Deliberately excludes text/html and image/svg+xml, which can carry script.
const INLINE_MIMES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "application/pdf",
];

fn is_inline_safe(mime: &str) -> bool {
    INLINE_MIMES.contains(&mime)
}

/// Validate and normalize a client-supplied MIME type, dropping any parameters
/// (e.g. `; charset=…`). Falls back to `application/octet-stream` when the value
/// is missing or not a well-formed `type/subtype` token.
fn sanitize_mime(raw: Option<&str>) -> String {
    let base = raw.unwrap_or("").split(';').next().unwrap_or("").trim();
    let is_token = |s: &str| {
        !s.is_empty()
            && s.chars().all(|c| {
                c.is_ascii_alphanumeric()
                    || matches!(c, '!' | '#' | '$' | '&' | '-' | '^' | '_' | '.' | '+')
            })
    };
    match base.split_once('/') {
        Some((t, sub)) if base.len() <= 255 && is_token(t) && is_token(sub) => base.to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

/// Build an RFC 6266 `Content-Disposition` value with an ASCII `filename`
/// fallback and a UTF-8 `filename*` for non-ASCII names (so the header is always
/// emittable as a valid `HeaderValue`).
fn content_disposition(kind: &str, name: &str) -> String {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_control() && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded = rfc5987_encode(name);
    format!("{kind}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

/// Percent-encode per RFC 5987 (unreserved chars pass through).
fn rfc5987_encode(s: &str) -> String {
    let mut out = String::new();
    for &byte in s.as_bytes() {
        let c = byte as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

/// Reduce an uploaded filename to a safe display name (no path, no control
/// chars/quotes, length-capped). The stored object key is a UUID regardless.
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(255)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}
