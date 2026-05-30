//! The MCP tool surface for Baitler.
//!
//! Each tool maps onto the same repositories and helpers the HTTP API uses, so
//! an MCP client gets the exact behaviour (owner-scoping, validation, HTML
//! sanitization, the shared export pathway) as a REST caller. Tools are scoped
//! to the single dev owner today; when auth lands ([`crate::owner`]), this is
//! the only place that needs to learn the real owner.

use std::collections::HashSet;

use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::ai::repo as ai_repo;
use crate::convert::{self, SourceFormat, TargetFormat};
use crate::crypto;
use crate::documents::model::{DocumentDto, DocumentSummary};
use crate::documents::repo as doc_repo;
use crate::error::AppError;
use crate::files::model::{FileDto, FolderDto};
use crate::files::repo as files_repo;
use crate::ideas::model::{IdeaDto, IdeaSummary, STATUSES};
use crate::ideas::repo as ideas_repo;
use crate::llm::{ChatMessage, ChatRequest};
use crate::owner::DEV_OWNER;
use crate::state::AppState;

use super::b64;

/// An error from executing a tool.
pub enum ToolError {
    /// No tool with the requested name (→ JSON-RPC method-not-found).
    UnknownTool,
    /// The arguments were missing/ill-typed or semantically invalid. Surfaced
    /// to the model as an `isError` tool result so it can correct itself.
    Invalid(String),
    /// An underlying repository/storage/convert error. Its `Display` is already
    /// client-safe (internal/DB details are collapsed to generic text).
    App(AppError),
}

impl From<AppError> for ToolError {
    fn from(e: AppError) -> Self {
        ToolError::App(e)
    }
}

type ToolResult = Result<Value, ToolError>;

fn invalid(msg: impl Into<String>) -> ToolError {
    ToolError::Invalid(msg.into())
}

fn not_found() -> ToolError {
    ToolError::App(AppError::NotFound)
}

// Limits mirror the HTTP layer so behaviour matches across surfaces.
const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;
const MAX_TITLE: usize = 200;
const MAX_TAGS: usize = 50;
const MAX_TAG_LEN: usize = 50;
const MAX_IDEA_BODY: usize = 1_000_000;
const MAX_DOC_BODY: usize = 5_000_000;
/// Cap on bytes moved through a single read/write tool call (Base64 in JSON is
/// memory-heavy; larger blobs should use the HTTP upload/download endpoints).
const MAX_BLOB: usize = 24 * 1024 * 1024;

// ── Dispatch ────────────────────────────────────────────────────────────────

/// Execute a tool by name. Returns the raw tool result value; the protocol
/// layer wraps it into MCP `content`.
pub async fn call(state: &AppState, name: &str, args: &Value) -> ToolResult {
    match name {
        "health" => health(state).await,
        // Ideas
        "ideas_list" => ideas_list(state, args).await,
        "ideas_get" => ideas_get(state, args).await,
        "ideas_create" => ideas_create(state, args).await,
        "ideas_update" => ideas_update(state, args).await,
        "ideas_delete" => ideas_delete(state, args).await,
        "ideas_link" => ideas_link(state, args).await,
        "ideas_unlink" => ideas_unlink(state, args).await,
        "ideas_tags" => ideas_tags(state).await,
        // Documents
        "documents_list" => documents_list(state).await,
        "documents_get" => documents_get(state, args).await,
        "documents_create" => documents_create(state, args).await,
        "documents_update" => documents_update(state, args).await,
        "documents_delete" => documents_delete(state, args).await,
        "documents_export" => documents_export(state, args).await,
        // Files & folders
        "files_list" => files_list(state, args).await,
        "files_get" => files_get(state, args).await,
        "files_read" => files_read(state, args).await,
        "files_write" => files_write(state, args).await,
        "files_delete" => files_delete(state, args).await,
        "folders_create" => folders_create(state, args).await,
        // AI
        "ai_providers" => ai_providers(state).await,
        "ai_chat" => ai_chat(state, args).await,
        // Conversion / export
        "export" => export_tool(args).await,
        _ => Err(ToolError::UnknownTool),
    }
}

// ── Argument helpers ──────────────────────────────────────────────────────────

fn req_str(args: &Value, key: &str) -> Result<String, ToolError> {
    match args.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.clone()),
        Some(Value::String(_)) => Err(invalid(format!("field `{key}` must not be empty"))),
        _ => Err(invalid(format!("missing or non-string field `{key}`"))),
    }
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// A present, trimmed, non-empty string field; otherwise `None`.
fn opt_trimmed(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn opt_usize(args: &Value, key: &str) -> Option<usize> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as usize)
}

fn opt_f32(args: &Value, key: &str) -> Option<f32> {
    args.get(key).and_then(|v| v.as_f64()).map(|n| n as f32)
}

fn opt_u32(args: &Value, key: &str) -> Option<u32> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as u32)
}

/// A present array-of-strings field (non-string entries are dropped).
fn opt_str_array(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect()
    })
}

// ── Validation (mirrors the HTTP handlers) ────────────────────────────────────

fn clean_title(title: &str) -> Result<String, ToolError> {
    let t = title.trim();
    if t.is_empty() {
        return Err(invalid("title must not be empty"));
    }
    if t.chars().count() > MAX_TITLE {
        return Err(invalid("title is too long"));
    }
    Ok(t.to_string())
}

fn clean_status(status: &str) -> Result<String, ToolError> {
    if STATUSES.contains(&status) {
        Ok(status.to_string())
    } else {
        Err(invalid(format!(
            "invalid status (expected one of: {})",
            STATUSES.join(", ")
        )))
    }
}

fn clean_tags(tags: Vec<String>) -> Result<Vec<String>, ToolError> {
    let mut cleaned: Vec<String> = Vec::new();
    for tag in tags {
        let t = tag.trim();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > MAX_TAG_LEN {
            return Err(invalid("a tag is too long"));
        }
        let t = t.to_string();
        if !cleaned.contains(&t) {
            cleaned.push(t);
        }
    }
    if cleaned.len() > MAX_TAGS {
        return Err(invalid("too many tags"));
    }
    Ok(cleaned)
}

fn clean_name(name: &str) -> Result<String, ToolError> {
    let t = name.trim();
    if t.is_empty() {
        return Err(invalid("name must not be empty"));
    }
    if t.len() > 255 || t.contains(['/', '\\', '\0']) {
        return Err(invalid("name contains invalid characters"));
    }
    Ok(t.to_string())
}

/// Reduce a free-form name to a safe export filename stem.
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
    let t = cleaned.trim();
    if t.is_empty() {
        "export".to_string()
    } else {
        t.to_string()
    }
}

/// Fold optional grounding context into the system prompt (mirrors `ai::routes`).
fn build_system(system: Option<String>, context: Option<String>) -> Option<String> {
    match context {
        Some(ctx) if !ctx.trim().is_empty() => {
            let base = system.unwrap_or_default();
            Some(
                format!("{base}\n\nUse the following context to answer the user:\n\n{ctx}")
                    .trim()
                    .to_string(),
            )
        }
        _ => system,
    }
}

// ── System ────────────────────────────────────────────────────────────────────

async fn health(state: &AppState) -> ToolResult {
    let db_up = crate::db::ping(&state.db).await.is_ok();
    Ok(json!({
        "status": if db_up { "ok" } else { "degraded" },
        "db": if db_up { "up" } else { "down" },
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ── Ideas ─────────────────────────────────────────────────────────────────────

async fn ideas_list(state: &AppState, args: &Value) -> ToolResult {
    let status = match opt_trimmed(args, "status") {
        Some(s) => Some(clean_status(&s)?),
        None => None,
    };
    let tag = opt_trimmed(args, "tag");
    let q = opt_trimmed(args, "q");
    let limit = opt_usize(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let offset = opt_usize(args, "offset").unwrap_or(0);

    let ideas = ideas_repo::list_ideas(
        &state.db,
        DEV_OWNER,
        status.as_deref(),
        tag.as_deref(),
        q.as_deref(),
        limit,
        offset,
    )
    .await?;
    let dtos: Vec<IdeaDto> = ideas.into_iter().map(Into::into).collect();
    Ok(json!({ "ideas": dtos }))
}

async fn ideas_get(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let idea = ideas_repo::get_idea(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    let related = ideas_repo::resolve_many(&state.db, DEV_OWNER, &idea.links).await?;
    let related: Vec<IdeaSummary> = related.into_iter().map(IdeaSummary::from).collect();
    Ok(json!({ "idea": IdeaDto::from(idea), "related": related }))
}

async fn ideas_create(state: &AppState, args: &Value) -> ToolResult {
    let title = clean_title(&req_str(args, "title")?)?;
    let body = opt_str(args, "body").unwrap_or_default();
    if body.len() > MAX_IDEA_BODY {
        return Err(invalid("body is too large"));
    }
    let tags = clean_tags(opt_str_array(args, "tags").unwrap_or_default())?;
    let status = match opt_trimmed(args, "status") {
        Some(s) => clean_status(&s)?,
        None => "inbox".to_string(),
    };
    let idea = ideas_repo::create_idea(&state.db, DEV_OWNER, &title, &body, &tags, &status).await?;
    Ok(json!(IdeaDto::from(idea)))
}

async fn ideas_update(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    ideas_repo::get_idea(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;

    let title = match opt_str(args, "title") {
        Some(t) => Some(clean_title(&t)?),
        None => None,
    };
    let body = match opt_str(args, "body") {
        Some(b) if b.len() > MAX_IDEA_BODY => return Err(invalid("body is too large")),
        other => other,
    };
    let tags = match opt_str_array(args, "tags") {
        Some(t) => Some(clean_tags(t)?),
        None => None,
    };
    let status = match opt_trimmed(args, "status") {
        Some(s) => Some(clean_status(&s)?),
        None => None,
    };

    let updated = ideas_repo::update_idea(
        &state.db,
        DEV_OWNER,
        &id,
        title.as_deref(),
        body.as_deref(),
        tags.as_deref(),
        status.as_deref(),
    )
    .await?
    .ok_or_else(not_found)?;
    Ok(json!(IdeaDto::from(updated)))
}

async fn ideas_delete(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if ideas_repo::delete_idea(&state.db, DEV_OWNER, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn ideas_link(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let target = req_str(args, "target_id")?;
    if id == target {
        return Err(invalid("an idea cannot link to itself"));
    }
    ideas_repo::get_idea(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    ideas_repo::link_ideas(&state.db, DEV_OWNER, &id, &target).await?;
    Ok(json!({ "linked": true, "id": id, "target_id": target }))
}

async fn ideas_unlink(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let target = req_str(args, "target_id")?;
    ideas_repo::get_idea(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    ideas_repo::unlink_ideas(&state.db, DEV_OWNER, &id, &target).await?;
    Ok(json!({ "unlinked": true, "id": id, "target_id": target }))
}

async fn ideas_tags(state: &AppState) -> ToolResult {
    let tags = ideas_repo::distinct_tags(&state.db, DEV_OWNER).await?;
    Ok(json!({ "tags": tags }))
}

// ── Documents ─────────────────────────────────────────────────────────────────

async fn documents_list(state: &AppState) -> ToolResult {
    let docs = doc_repo::list_documents(&state.db, DEV_OWNER).await?;
    let dtos: Vec<DocumentSummary> = docs.into_iter().map(Into::into).collect();
    Ok(json!({ "documents": dtos }))
}

async fn documents_get(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let doc = doc_repo::get_document(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(DocumentDto::from(doc)))
}

async fn documents_create(state: &AppState, args: &Value) -> ToolResult {
    let title = clean_title(&req_str(args, "title")?)?;
    let raw = opt_str(args, "body").unwrap_or_default();
    if raw.len() > MAX_DOC_BODY {
        return Err(invalid("document is too large"));
    }
    let html = convert::sanitize(&raw);
    let doc = doc_repo::create_document(&state.db, DEV_OWNER, &title, &html).await?;
    Ok(json!(DocumentDto::from(doc)))
}

async fn documents_update(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    doc_repo::get_document(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;

    let title = match opt_str(args, "title") {
        Some(t) => Some(clean_title(&t)?),
        None => None,
    };
    let html = match opt_str(args, "body") {
        Some(b) if b.len() > MAX_DOC_BODY => return Err(invalid("document is too large")),
        Some(b) => Some(convert::sanitize(&b)),
        None => None,
    };

    let updated =
        doc_repo::update_document(&state.db, DEV_OWNER, &id, title.as_deref(), html.as_deref())
            .await?
            .ok_or_else(not_found)?;
    Ok(json!(DocumentDto::from(updated)))
}

async fn documents_delete(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if doc_repo::delete_document(&state.db, DEV_OWNER, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn documents_export(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let target = TargetFormat::parse(&req_str(args, "format")?)
        .ok_or_else(|| invalid("unsupported export format (html|markdown|pdf|docx)"))?;
    let doc = doc_repo::get_document(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    let bytes = convert::export(&doc.body, SourceFormat::Html, target)
        .await
        .map_err(AppError::from)?;
    Ok(export_payload(bytes, target, &doc.title))
}

// ── Conversion / export ───────────────────────────────────────────────────────

async fn export_tool(args: &Value) -> ToolResult {
    let content = req_str(args, "content")?;
    if content.len() > MAX_DOC_BODY {
        return Err(invalid("content is too large"));
    }
    let source = SourceFormat::parse(&req_str(args, "source")?)
        .ok_or_else(|| invalid("unsupported source format (html|markdown)"))?;
    let target = TargetFormat::parse(&req_str(args, "target")?)
        .ok_or_else(|| invalid("unsupported target format (html|markdown|pdf|docx)"))?;
    let name = opt_trimmed(args, "filename").unwrap_or_else(|| "export".to_string());
    let bytes = convert::export(&content, source, target)
        .await
        .map_err(AppError::from)?;
    Ok(export_payload(bytes, target, &name))
}

/// Shape an export result: always Base64 + metadata; for text targets also
/// include a decoded `text` field for convenience.
fn export_payload(bytes: Vec<u8>, target: TargetFormat, name: &str) -> Value {
    let is_text = matches!(target, TargetFormat::Html | TargetFormat::Markdown);
    let mut out = json!({
        "filename": format!("{}.{}", sanitize_filename(name), target.extension()),
        "content_type": target.content_type(),
        "bytes": bytes.len(),
        "encoding": "base64",
        "content_base64": b64::encode(&bytes),
    });
    if is_text {
        if let Ok(text) = String::from_utf8(bytes) {
            out["text"] = json!(text);
        }
    }
    out
}

// ── Files & folders ───────────────────────────────────────────────────────────

async fn files_list(state: &AppState, args: &Value) -> ToolResult {
    let db = &state.db;

    if let Some(q) = opt_trimmed(args, "q") {
        let limit = opt_usize(args, "limit")
            .unwrap_or(DEFAULT_LIMIT)
            .min(MAX_LIMIT);
        let offset = opt_usize(args, "offset").unwrap_or(0);
        let files = files_repo::search_files(db, DEV_OWNER, &q, limit, offset).await?;
        let files: Vec<FileDto> = files.into_iter().map(Into::into).collect();
        return Ok(json!({ "files": files, "folders": [] }));
    }

    let folder = opt_trimmed(args, "folder");
    if let Some(f) = folder.as_deref() {
        files_repo::get_folder(db, DEV_OWNER, f)
            .await?
            .ok_or_else(not_found)?;
    }
    let folders = files_repo::list_folders(db, DEV_OWNER, folder.as_deref()).await?;
    let files = files_repo::list_files(db, DEV_OWNER, folder.as_deref()).await?;
    let folders: Vec<FolderDto> = folders.into_iter().map(Into::into).collect();
    let files: Vec<FileDto> = files.into_iter().map(Into::into).collect();
    Ok(json!({ "folder": folder, "folders": folders, "files": files }))
}

async fn files_get(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let file = files_repo::get_file(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(FileDto::from(file)))
}

async fn files_read(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let file = files_repo::get_file(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    if file.size as usize > MAX_BLOB {
        return Err(invalid(format!(
            "file is {} bytes; too large to read over MCP (limit {MAX_BLOB}). \
             Use the HTTP download endpoint GET /files/{id}/content instead.",
            file.size
        )));
    }
    let mut reader = state
        .storage
        .open(&file.storage_key)
        .await
        .map_err(AppError::from)?;
    let mut bytes = Vec::with_capacity(file.size.max(0) as usize);
    reader
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| ToolError::App(AppError::Internal(Box::new(e))))?;

    Ok(json!({
        "file": FileDto::from(file),
        "encoding": "base64",
        "content_base64": b64::encode(&bytes),
    }))
}

async fn files_write(state: &AppState, args: &Value) -> ToolResult {
    let db = &state.db;
    let name = clean_name(&req_str(args, "name")?)?;
    let folder = opt_trimmed(args, "folder");
    if let Some(f) = folder.as_deref() {
        files_repo::get_folder(db, DEV_OWNER, f)
            .await?
            .ok_or_else(|| invalid("target folder does not exist"))?;
    }

    let bytes: Vec<u8> = if let Some(b64s) = opt_str(args, "content_base64") {
        b64::decode(&b64s).map_err(|e| invalid(format!("invalid base64 content: {e}")))?
    } else if let Some(text) = opt_str(args, "content_text") {
        text.into_bytes()
    } else {
        return Err(invalid(
            "provide file contents as `content_base64` or `content_text`",
        ));
    };
    if bytes.len() > MAX_BLOB {
        return Err(invalid(format!(
            "content is {} bytes; exceeds the MCP write limit ({MAX_BLOB})",
            bytes.len()
        )));
    }
    let mime = opt_trimmed(args, "mime").unwrap_or_else(|| "application/octet-stream".to_string());

    let storage_key = Uuid::new_v4().to_string();
    state
        .storage
        .put(&storage_key, &bytes)
        .await
        .map_err(AppError::from)?;

    match files_repo::create_file(
        db,
        DEV_OWNER,
        &storage_key,
        &name,
        &mime,
        bytes.len() as i64,
        folder.as_deref(),
        &storage_key,
    )
    .await
    {
        Ok(file) => Ok(json!(FileDto::from(file))),
        Err(e) => {
            // Roll back the orphaned object so a failed metadata write leaves no trace.
            let _ = state.storage.delete(&storage_key).await;
            Err(e.into())
        }
    }
}

async fn files_delete(state: &AppState, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let deleted = files_repo::delete_file(&state.db, DEV_OWNER, &id)
        .await?
        .ok_or_else(not_found)?;
    if let Err(e) = state.storage.delete(&deleted.storage_key).await {
        tracing::warn!(error = %e, key = %deleted.storage_key, "mcp files_delete: storage cleanup failed");
    }
    Ok(json!({ "deleted": true, "id": id }))
}

async fn folders_create(state: &AppState, args: &Value) -> ToolResult {
    let db = &state.db;
    let name = clean_name(&req_str(args, "name")?)?;
    let parent = opt_trimmed(args, "parent_id");
    if let Some(p) = parent.as_deref() {
        files_repo::get_folder(db, DEV_OWNER, p)
            .await?
            .ok_or_else(|| invalid("parent folder does not exist"))?;
    }
    let folder = files_repo::create_folder(db, DEV_OWNER, &name, parent.as_deref()).await?;
    Ok(json!(FolderDto::from(folder)))
}

// ── AI ────────────────────────────────────────────────────────────────────────

async fn ai_providers(state: &AppState) -> ToolResult {
    let configured: HashSet<String> = ai_repo::list_configured(&state.db, DEV_OWNER)
        .await?
        .into_iter()
        .collect();
    let providers: Vec<Value> = state
        .llm
        .all()
        .iter()
        .map(|p| {
            json!({
                "id": p.id(),
                "label": p.label(),
                "requires_key": p.requires_key(),
                "configured": !p.requires_key() || configured.contains(p.id()),
                "models": p.models(),
            })
        })
        .collect();
    Ok(json!({ "providers": providers }))
}

async fn ai_chat(state: &AppState, args: &Value) -> ToolResult {
    let provider_id = req_str(args, "provider")?;
    let model = req_str(args, "model")?;
    let messages: Vec<ChatMessage> = match args.get("messages") {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| invalid(format!("invalid `messages`: {e}")))?,
        None => return Err(invalid("missing `messages`")),
    };
    if messages.is_empty() {
        return Err(invalid("`messages` must not be empty"));
    }

    let provider = state
        .llm
        .get(&provider_id)
        .ok_or_else(|| invalid(format!("unknown provider `{provider_id}`")))?;

    let api_key: Option<String> = if provider.requires_key() {
        let ciphertext = ai_repo::get_ciphertext(&state.db, DEV_OWNER, provider.id())
            .await?
            .ok_or_else(|| invalid(format!("no API key configured for {}", provider.id())))?;
        let key = crypto::decrypt(&state.config.secret_key, &ciphertext)
            .map_err(|e| ToolError::App(AppError::Internal(Box::new(e))))?;
        Some(key)
    } else {
        None
    };

    let req = ChatRequest {
        model,
        messages,
        system: build_system(opt_str(args, "system"), opt_str(args, "context")),
        temperature: opt_f32(args, "temperature"),
        max_tokens: opt_u32(args, "max_tokens"),
    };

    // The HTTP API streams; an agent tool call wants the whole answer, so we
    // collect the token stream into one string.
    let mut stream = provider
        .chat_stream(req, api_key.as_deref())
        .await
        .map_err(AppError::from)?;
    let mut text = String::new();
    let mut stream_error: Option<String> = None;
    while let Some(item) = stream.next().await {
        match item {
            Ok(delta) => text.push_str(&delta),
            Err(e) => {
                stream_error = Some(e.to_string());
                break;
            }
        }
    }

    let mut out = json!({ "provider": provider_id, "text": text });
    if let Some(e) = stream_error {
        out["error"] = json!(e);
    }
    Ok(out)
}

// ── Tool catalog (advertised via `tools/list`) ────────────────────────────────

/// Build one tool definition.
fn def(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        },
    })
}

fn str_schema(desc: &str) -> Value {
    json!({ "type": "string", "description": desc })
}

fn int_schema(desc: &str) -> Value {
    json!({ "type": "integer", "description": desc })
}

/// The full set of tools advertised to MCP clients.
pub fn definitions() -> Vec<Value> {
    let status_desc = format!("one of: {}", STATUSES.join(", "));
    vec![
        def("health", "Service and database readiness check.", json!({}), &[]),
        // Ideas
        def(
            "ideas_list",
            "List ideas (notes with Markdown bodies, tags, status, links). Optional filters.",
            json!({
                "status": str_schema(&status_desc),
                "tag": str_schema("only ideas carrying this tag"),
                "q": str_schema("full-text search over title and body"),
                "limit": int_schema("max results (default 100, max 500)"),
                "offset": int_schema("pagination offset"),
            }),
            &[],
        ),
        def(
            "ideas_get",
            "Fetch a single idea by id, including its related (linked) ideas.",
            json!({ "id": str_schema("idea id") }),
            &["id"],
        ),
        def(
            "ideas_create",
            "Create a new idea. Body is Markdown.",
            json!({
                "title": str_schema("idea title (required)"),
                "body": str_schema("Markdown body"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "tags" }),
                "status": str_schema(&status_desc),
            }),
            &["title"],
        ),
        def(
            "ideas_update",
            "Update fields of an existing idea. Only provided fields change.",
            json!({
                "id": str_schema("idea id (required)"),
                "title": str_schema("new title"),
                "body": str_schema("new Markdown body"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "replacement tag set" }),
                "status": str_schema(&status_desc),
            }),
            &["id"],
        ),
        def(
            "ideas_delete",
            "Delete an idea by id.",
            json!({ "id": str_schema("idea id") }),
            &["id"],
        ),
        def(
            "ideas_link",
            "Create a (symmetric) link between two ideas.",
            json!({
                "id": str_schema("source idea id"),
                "target_id": str_schema("idea id to link to"),
            }),
            &["id", "target_id"],
        ),
        def(
            "ideas_unlink",
            "Remove the link between two ideas.",
            json!({
                "id": str_schema("source idea id"),
                "target_id": str_schema("linked idea id to remove"),
            }),
            &["id", "target_id"],
        ),
        def(
            "ideas_tags",
            "List all distinct tags used across the owner's ideas.",
            json!({}),
            &[],
        ),
        // Documents
        def(
            "documents_list",
            "List HTML documents (id, title, version, updated_at).",
            json!({}),
            &[],
        ),
        def(
            "documents_get",
            "Fetch a single document (including its HTML body) by id.",
            json!({ "id": str_schema("document id") }),
            &["id"],
        ),
        def(
            "documents_create",
            "Create an HTML document. The body is sanitized server-side.",
            json!({
                "title": str_schema("document title (required)"),
                "body": str_schema("HTML body"),
            }),
            &["title"],
        ),
        def(
            "documents_update",
            "Update a document's title and/or HTML body (sanitized).",
            json!({
                "id": str_schema("document id (required)"),
                "title": str_schema("new title"),
                "body": str_schema("new HTML body"),
            }),
            &["id"],
        ),
        def(
            "documents_delete",
            "Delete a document by id.",
            json!({ "id": str_schema("document id") }),
            &["id"],
        ),
        def(
            "documents_export",
            "Export a stored document to html, markdown, pdf, or docx. \
             Binary results come back Base64-encoded (pdf needs Chrome, docx needs Pandoc).",
            json!({
                "id": str_schema("document id"),
                "format": str_schema("html | markdown | pdf | docx"),
            }),
            &["id", "format"],
        ),
        // Files & folders
        def(
            "files_list",
            "List a folder's files and subfolders, or search files when `q` is set.",
            json!({
                "folder": str_schema("folder id; omit for the root"),
                "q": str_schema("search files by name across all folders"),
                "limit": int_schema("max search results (default 100, max 500)"),
                "offset": int_schema("search pagination offset"),
            }),
            &[],
        ),
        def(
            "files_get",
            "Fetch a file's metadata by id.",
            json!({ "id": str_schema("file id") }),
            &["id"],
        ),
        def(
            "files_read",
            "Read a file's contents, returned Base64-encoded (size-limited).",
            json!({ "id": str_schema("file id") }),
            &["id"],
        ),
        def(
            "files_write",
            "Create a file from inline content (Base64 or UTF-8 text), optionally in a folder.",
            json!({
                "name": str_schema("file name (required)"),
                "content_base64": str_schema("file bytes, Base64-encoded"),
                "content_text": str_schema("file contents as UTF-8 text (alternative to content_base64)"),
                "mime": str_schema("MIME type (default application/octet-stream)"),
                "folder": str_schema("destination folder id; omit for the root"),
            }),
            &["name"],
        ),
        def(
            "files_delete",
            "Delete a file by id.",
            json!({ "id": str_schema("file id") }),
            &["id"],
        ),
        def(
            "folders_create",
            "Create a folder, optionally nested under a parent.",
            json!({
                "name": str_schema("folder name (required)"),
                "parent_id": str_schema("parent folder id; omit for the root"),
            }),
            &["name"],
        ),
        // AI
        def(
            "ai_providers",
            "List configured LLM providers and their models.",
            json!({}),
            &[],
        ),
        def(
            "ai_chat",
            "Run a non-streaming chat completion via a configured provider and return the full text.",
            json!({
                "provider": str_schema("provider id (e.g. mock, openai, anthropic, openrouter)"),
                "model": str_schema("model id from ai_providers"),
                "messages": json!({
                    "type": "array",
                    "description": "chat messages",
                    "items": {
                        "type": "object",
                        "properties": {
                            "role": { "type": "string", "enum": ["system", "user", "assistant"] },
                            "content": { "type": "string" },
                        },
                        "required": ["role", "content"],
                    },
                }),
                "system": str_schema("system prompt"),
                "context": str_schema("optional grounding text folded into the system prompt"),
                "temperature": json!({ "type": "number", "description": "sampling temperature" }),
                "max_tokens": int_schema("max tokens to generate"),
            }),
            &["provider", "model", "messages"],
        ),
        // Conversion / export
        def(
            "export",
            "Convert arbitrary content between formats via the shared export pathway. \
             Binary targets come back Base64-encoded.",
            json!({
                "content": str_schema("the source content"),
                "source": str_schema("html | markdown"),
                "target": str_schema("html | markdown | pdf | docx"),
                "filename": str_schema("base name for the result file (optional)"),
            }),
            &["content", "source", "target"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_advertised_tool_is_dispatchable_by_name() {
        // Guards against advertising a tool name that `call` doesn't handle.
        // We can't call them without a DB here, but we can assert the names are
        // known by checking `call` returns something other than UnknownTool for
        // a syntactically present name — done via the explicit name list below.
        let advertised: Vec<String> = definitions()
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_string())
            .collect();
        let known = [
            "health",
            "ideas_list",
            "ideas_get",
            "ideas_create",
            "ideas_update",
            "ideas_delete",
            "ideas_link",
            "ideas_unlink",
            "ideas_tags",
            "documents_list",
            "documents_get",
            "documents_create",
            "documents_update",
            "documents_delete",
            "documents_export",
            "files_list",
            "files_get",
            "files_read",
            "files_write",
            "files_delete",
            "folders_create",
            "ai_providers",
            "ai_chat",
            "export",
        ];
        for name in &advertised {
            assert!(known.contains(&name.as_str()), "undispatched tool: {name}");
        }
        assert_eq!(advertised.len(), known.len(), "tool count drifted");
    }

    #[test]
    fn tool_schemas_are_well_formed() {
        for d in definitions() {
            assert!(d["name"].is_string());
            assert!(d["description"].is_string());
            assert_eq!(d["inputSchema"]["type"], "object");
            assert!(d["inputSchema"]["properties"].is_object());
            assert!(d["inputSchema"]["required"].is_array());
        }
    }

    #[test]
    fn req_str_rejects_missing_and_empty() {
        let args = json!({ "a": "x", "b": "  " });
        assert!(req_str(&args, "a").is_ok());
        assert!(req_str(&args, "b").is_err());
        assert!(req_str(&args, "missing").is_err());
    }
}
