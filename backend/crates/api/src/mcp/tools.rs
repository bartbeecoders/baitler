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

use crate::activity;
use crate::ai::repo as ai_repo;
use crate::cli::model::{CliRunDto, CliRunSummary};
use crate::cli::repo as cli_repo;
use crate::convert::{self, SourceFormat, TargetFormat};
use crate::crypto;
use crate::diagrams::model::{DiagramDto, DiagramSummary};
use crate::diagrams::repo::{self as diagrams_repo, DiagramPatch};
use crate::documents::model::{DocumentDto, DocumentSummary};
use crate::documents::repo as doc_repo;
use crate::error::AppError;
use crate::files::model::{FileDto, FolderDto};
use crate::files::repo as files_repo;
use crate::ideas::model::{IdeaDto, IdeaSummary, REVIEWS, STATUSES};
use crate::ideas::repo as ideas_repo;

use crate::knowledge::model::{ProjectDto, ReviewQueue, PROJECT_STATUSES};
use crate::knowledge::repo as kn;
use crate::llm::{ChatMessage, ChatRequest};
use crate::mindmap::model::{
    from_markdown_outline, Graph, MindmapDto, MindmapSummary,
    SOURCE_FORMATS as MINDMAP_SOURCE_FORMATS,
};
use crate::mindmap::repo::{self as mindmap_repo, MindmapPatch};
use crate::pages::model::{PageDto, PageSummary, SOURCE_FORMATS, VISIBILITIES};
use crate::pages::repo::{self as pages_repo, PagePatch};
use crate::state::AppState;
use crate::superpage::context;
use crate::superpage::model::{Layout, SuperpageDto, SuperpageSummary};
use crate::superpage::repo as superpage_repo;

use super::{b64, Actor};

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
/// Link/search endpoint types (mirrors [`crate::knowledge::model::ITEM_TYPES`]).
const ITEM_TYPES_DESC: &str = "idea | document | file | project | page | mindmap | diagram";
/// Project membership types (mirrors [`crate::knowledge::model::MEMBER_TYPES`]).
const MEMBER_TYPES_DESC: &str = "idea | document | file | page | mindmap | diagram";
const CLI_DEFAULT_LIMIT: usize = 50;
const CLI_MAX_LIMIT: usize = 200;
const ANTHROPIC_PROVIDER: &str = "anthropic";

// ── Dispatch ────────────────────────────────────────────────────────────────

/// Execute a tool by name. Returns the raw tool result value; the protocol
/// layer wraps it into MCP `content`.
///
/// Takes the full [`Actor`] (not just the owner) so the plugin dispatch path
/// (Phase 16) carries provenance and run-gating; the static tools below remain
/// owner-scoped.
pub(crate) async fn call(state: &AppState, actor: &Actor, name: &str, args: &Value) -> ToolResult {
    let owner = actor.owner.as_str();
    match name {
        "health" => health(state).await,
        // Ideas
        "ideas_list" => ideas_list(state, owner, args).await,
        "ideas_get" => ideas_get(state, owner, args).await,
        "ideas_create" => ideas_create(state, owner, args).await,
        "ideas_update" => ideas_update(state, owner, args).await,
        "ideas_delete" => ideas_delete(state, owner, args).await,
        "ideas_link" => ideas_link(state, owner, args).await,
        "ideas_unlink" => ideas_unlink(state, owner, args).await,
        "ideas_tags" => ideas_tags(state, owner).await,
        // Documents
        "documents_list" => documents_list(state, owner, args).await,
        "documents_get" => documents_get(state, owner, args).await,
        "documents_create" => documents_create(state, owner, args).await,
        "documents_update" => documents_update(state, owner, args).await,
        "documents_delete" => documents_delete(state, owner, args).await,
        "documents_export" => documents_export(state, owner, args).await,
        // Files & folders
        "files_list" => files_list(state, owner, args).await,
        "files_get" => files_get(state, owner, args).await,
        "files_read" => files_read(state, owner, args).await,
        "files_write" => files_write(state, owner, args).await,
        "files_import" => files_import(state, owner, args).await,
        "files_delete" => files_delete(state, owner, args).await,
        "folders_create" => folders_create(state, owner, args).await,
        // Projects
        "projects_list" => projects_list(state, owner).await,
        "projects_get" => projects_get(state, owner, args).await,
        "projects_create" => projects_create(state, owner, args).await,
        "projects_update" => projects_update(state, owner, args).await,
        "projects_delete" => projects_delete(state, owner, args).await,
        "projects_add_item" => projects_add_item(state, owner, args).await,
        "projects_remove_item" => projects_remove_item(state, owner, args).await,
        // Pages (hosted web pages)
        "pages_list" => pages_list(state, owner, args).await,
        "pages_get" => pages_get(state, owner, args).await,
        "pages_create" => pages_create(state, owner, args).await,
        "pages_update" => pages_update(state, owner, args).await,
        "pages_delete" => pages_delete(state, owner, args).await,
        "pages_publish" => pages_publish(state, owner, args).await,
        "pages_unpublish" => pages_unpublish(state, owner, args).await,
        // Mindmaps (visual idea maps)
        "mindmaps_list" => mindmaps_list(state, owner, args).await,
        "mindmaps_get" => mindmaps_get(state, owner, args).await,
        "mindmaps_create" => mindmaps_create(state, owner, args).await,
        "mindmaps_update" => mindmaps_update(state, owner, args).await,
        "mindmaps_delete" => mindmaps_delete(state, owner, args).await,
        "mindmaps_from_project" => mindmaps_from_project(state, owner, args).await,
        // Diagrams (draw.io / mxGraph)
        "diagrams_list" => diagrams_list(state, owner, args).await,
        "diagrams_get" => diagrams_get(state, owner, args).await,
        "diagrams_create" => diagrams_create(state, owner, args).await,
        "diagrams_update" => diagrams_update(state, owner, args).await,
        "diagrams_delete" => diagrams_delete(state, owner, args).await,
        // Superpages (composed canvas)
        "superpages_list" => superpages_list(state, owner, args).await,
        "superpages_get" => superpages_get(state, owner, args).await,
        "superpages_create" => superpages_create(state, owner, args).await,
        "superpages_update" => superpages_update(state, owner, args).await,
        "superpages_delete" => superpages_delete(state, owner, args).await,
        "superpages_context" => superpages_context(state, owner, args).await,
        "superpages_from_project" => superpages_from_project(state, owner, args).await,
        // Knowledge graph + search
        "knowledge_link" => knowledge_link(state, owner, args).await,
        "knowledge_unlink" => knowledge_unlink(state, owner, args).await,
        "knowledge_backlinks" => knowledge_backlinks(state, owner, args).await,
        "knowledge_search" => knowledge_search(state, owner, args).await,
        "knowledge_tags" => knowledge_tags(state, owner).await,
        // Publishing & export
        "documents_publish" => documents_publish(state, owner, args).await,
        "collection_export" => collection_export(state, owner, args).await,
        // Activity / provenance
        "activity_list" => activity_list(state, owner, args).await,
        "review_list" => review_list(state, owner).await,
        // Agent (Claude Code CLI)
        "cli_status" => cli_status(state, owner).await,
        "cli_runs_list" => cli_runs_list(state, owner, args).await,
        "cli_runs_get" => cli_runs_get(state, owner, args).await,
        "cli_run_cancel" => cli_run_cancel(state, owner, args).await,
        // Workspace (local disk, jailed to WORKSPACE_ROOTS). NOT available to
        // Baitler-spawned agent runs — the protocol layer rejects those calls
        // before dispatch (see `mcp::handle_tools_call`).
        "workspace_roots" => workspace_roots_tool(state).await,
        "workspace_list" => workspace_list(state, args).await,
        "workspace_info" => workspace_info(state, args).await,
        "workspace_read" => workspace_read(state, args).await,
        "workspace_write" => workspace_write(state, args).await,
        "workspace_mkdir" => workspace_mkdir(state, args).await,
        "workspace_delete" => workspace_delete(state, args).await,
        "workspace_rmdir" => workspace_rmdir(state, args).await,
        "workspace_move" => workspace_move(state, args).await,
        "workspace_copy" => workspace_copy(state, args).await,
        // AI
        "ai_providers" => ai_providers(state, owner).await,
        "ai_chat" => ai_chat(state, owner, args).await,
        // Conversion / export
        "export" => export_tool(args).await,
        // Plugin-provided tools (Phase 16): resolved by the runtime registry,
        // not this static match. The registry is empty until the 16.B loader
        // lands, so today this yields the same UnknownTool as before the seam.
        n if n.starts_with(crate::plugins::TOOL_PREFIX) => {
            state.plugins.dispatch(actor, n, args).await
        }
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

fn opt_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
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

/// The review state for an agent write. Defaults to `draft` (so agent-authored
/// content lands in the human review queue) unless an explicit, valid `review`
/// argument is supplied.
fn clean_review(args: &Value) -> Result<String, ToolError> {
    match opt_trimmed(args, "review") {
        Some(r) if REVIEWS.contains(&r.as_str()) => Ok(r),
        Some(_) => Err(invalid(
            "invalid review (expected one of: draft, published)",
        )),
        None => Ok("draft".to_string()),
    }
}

/// An optional explicit review transition for an update (None = leave unchanged).
fn opt_review(args: &Value) -> Result<Option<String>, ToolError> {
    match opt_trimmed(args, "review") {
        Some(r) if REVIEWS.contains(&r.as_str()) => Ok(Some(r)),
        Some(_) => Err(invalid(
            "invalid review (expected one of: draft, published)",
        )),
        None => Ok(None),
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

async fn ideas_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
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
        owner,
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

async fn ideas_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let idea = ideas_repo::get_idea(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    let related = ideas_repo::resolve_many(&state.db, owner, &idea.links).await?;
    let related: Vec<IdeaSummary> = related.into_iter().map(IdeaSummary::from).collect();
    Ok(json!({ "idea": IdeaDto::from(idea), "related": related }))
}

async fn ideas_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
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
    let review = clean_review(args)?;
    let project_id = resolve_project_arg(state, owner, args).await?;
    let idea = ideas_repo::create_idea(
        &state.db,
        owner,
        &title,
        &body,
        &tags,
        &status,
        &review,
        project_id.as_deref(),
    )
    .await?;
    Ok(json!(IdeaDto::from(idea)))
}

async fn ideas_update(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    ideas_repo::get_idea(&state.db, owner, &id)
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
    let review = opt_review(args)?;

    let updated = ideas_repo::update_idea(
        &state.db,
        owner,
        &id,
        title.as_deref(),
        body.as_deref(),
        tags.as_deref(),
        status.as_deref(),
        review.as_deref(),
    )
    .await?
    .ok_or_else(not_found)?;
    Ok(json!(IdeaDto::from(updated)))
}

async fn ideas_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if ideas_repo::delete_idea(&state.db, owner, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn ideas_link(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let target = req_str(args, "target_id")?;
    if id == target {
        return Err(invalid("an idea cannot link to itself"));
    }
    ideas_repo::get_idea(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    ideas_repo::link_ideas(&state.db, owner, &id, &target).await?;
    Ok(json!({ "linked": true, "id": id, "target_id": target }))
}

async fn ideas_unlink(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let target = req_str(args, "target_id")?;
    ideas_repo::get_idea(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    ideas_repo::unlink_ideas(&state.db, owner, &id, &target).await?;
    Ok(json!({ "unlinked": true, "id": id, "target_id": target }))
}

async fn ideas_tags(state: &AppState, owner: &str) -> ToolResult {
    let tags = ideas_repo::distinct_tags(&state.db, owner).await?;
    Ok(json!({ "tags": tags }))
}

// ── Documents ─────────────────────────────────────────────────────────────────

async fn documents_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let docs =
        doc_repo::list_documents(&state.db, owner, opt_trimmed(args, "tag").as_deref()).await?;
    let dtos: Vec<DocumentSummary> = docs.into_iter().map(Into::into).collect();
    Ok(json!({ "documents": dtos }))
}

async fn documents_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let doc = doc_repo::get_document(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(DocumentDto::from(doc)))
}

async fn documents_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let title = clean_title(&req_str(args, "title")?)?;
    let raw = opt_str(args, "body").unwrap_or_default();
    if raw.len() > MAX_DOC_BODY {
        return Err(invalid("document is too large"));
    }
    let html = convert::sanitize(&raw);
    let review = clean_review(args)?;
    let project_id = resolve_project_arg(state, owner, args).await?;
    let tags = clean_tags(opt_str_array(args, "tags").unwrap_or_default())?;
    let doc = doc_repo::create_document(
        &state.db,
        owner,
        &title,
        &html,
        &review,
        project_id.as_deref(),
        &tags,
    )
    .await?;
    Ok(json!(DocumentDto::from(doc)))
}

async fn documents_update(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    doc_repo::get_document(&state.db, owner, &id)
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
    let review = opt_review(args)?;
    let tags = match opt_str_array(args, "tags") {
        Some(t) => Some(clean_tags(t)?),
        None => None,
    };

    let updated = doc_repo::update_document(
        &state.db,
        owner,
        &id,
        title.as_deref(),
        html.as_deref(),
        review.as_deref(),
        tags.as_deref(),
    )
    .await?
    .ok_or_else(not_found)?;
    Ok(json!(DocumentDto::from(updated)))
}

async fn documents_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if doc_repo::delete_document(&state.db, owner, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn documents_export(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let target = TargetFormat::parse(&req_str(args, "format")?)
        .ok_or_else(|| invalid("unsupported export format (html|markdown|pdf|docx)"))?;
    let doc = doc_repo::get_document(&state.db, owner, &id)
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

async fn files_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;

    if let Some(q) = opt_trimmed(args, "q") {
        let limit = opt_usize(args, "limit")
            .unwrap_or(DEFAULT_LIMIT)
            .min(MAX_LIMIT);
        let offset = opt_usize(args, "offset").unwrap_or(0);
        let files = files_repo::search_files(db, owner, &q, limit, offset).await?;
        let files: Vec<FileDto> = files.into_iter().map(Into::into).collect();
        return Ok(json!({ "files": files, "folders": [] }));
    }

    let folder = opt_trimmed(args, "folder");
    if let Some(f) = folder.as_deref() {
        files_repo::get_folder(db, owner, f)
            .await?
            .ok_or_else(not_found)?;
    }
    let folders = files_repo::list_folders(db, owner, folder.as_deref()).await?;
    let files = files_repo::list_files(db, owner, folder.as_deref()).await?;
    let folders: Vec<FolderDto> = folders.into_iter().map(Into::into).collect();
    let files: Vec<FileDto> = files.into_iter().map(Into::into).collect();
    Ok(json!({ "folder": folder, "folders": folders, "files": files }))
}

async fn files_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let file = files_repo::get_file(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(FileDto::from(file)))
}

async fn files_read(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let file = files_repo::get_file(&state.db, owner, &id)
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

async fn files_write(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let name = clean_name(&req_str(args, "name")?)?;
    let folder = opt_trimmed(args, "folder");
    if let Some(f) = folder.as_deref() {
        files_repo::get_folder(db, owner, f)
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
        owner,
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

/// Per-extension MIME so imported images/docs get a sensible content type
/// (previews, downloads). Unknown → octet-stream.
fn mime_from_ext(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        Some("heic") => "image/heic",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// Upper bound on files imported in one `files_import` call (a runaway guard).
const MAX_IMPORT_FILES: usize = 500;

/// Import local files into Baitler Files, **server-side** — the server reads the
/// bytes from disk directly, so the agent never has to base64 binary content
/// through its context, and no host shell is needed. The path must resolve within
/// an allow-listed root (`CLAUDE_CLI_WORKSPACE_ROOTS`); a directory imports its
/// files (optionally recursive, symlinks not followed).
async fn files_import(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let path = req_str(args, "path")?;
    let folder = opt_trimmed(args, "folder");
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if let Some(f) = folder.as_deref() {
        files_repo::get_folder(db, owner, f)
            .await?
            .ok_or_else(|| invalid("target folder does not exist"))?;
    }

    let target =
        crate::cli::resolve_under_roots(&state.config.workspace_roots, &path).map_err(invalid)?;

    // Collect candidate files (a single file, or a directory walk).
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut truncated = false;
    if target.is_file() {
        files.push(target);
    } else if target.is_dir() {
        let mut stack = vec![target];
        'walk: while let Some(dir) = stack.pop() {
            let mut rd = tokio::fs::read_dir(&dir)
                .await
                .map_err(|e| invalid(format!("could not read directory: {e}")))?;
            while let Some(entry) = rd
                .next_entry()
                .await
                .map_err(|e| invalid(format!("could not read directory: {e}")))?
            {
                let Ok(ft) = entry.file_type().await else {
                    continue;
                };
                // Don't follow symlinks (could escape the allow-listed root).
                if ft.is_dir() {
                    if recursive {
                        stack.push(entry.path());
                    }
                } else if ft.is_file() {
                    files.push(entry.path());
                    if files.len() >= MAX_IMPORT_FILES {
                        truncated = true;
                        break 'walk;
                    }
                }
            }
        }
    } else {
        return Err(invalid("path is neither a file nor a directory"));
    }

    let mut imported: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    for p in files {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = match tokio::fs::read(&p).await {
            Ok(b) => b,
            Err(e) => {
                skipped.push(json!({ "name": name, "reason": format!("unreadable: {e}") }));
                continue;
            }
        };
        if bytes.len() > MAX_BLOB {
            skipped.push(json!({ "name": name, "reason": format!("exceeds {MAX_BLOB} bytes") }));
            continue;
        }
        let storage_key = Uuid::new_v4().to_string();
        if let Err(e) = state.storage.put(&storage_key, &bytes).await {
            skipped.push(json!({ "name": name, "reason": format!("storage error: {e}") }));
            continue;
        }
        match files_repo::create_file(
            db,
            owner,
            &storage_key,
            &name,
            mime_from_ext(&name),
            bytes.len() as i64,
            folder.as_deref(),
            &storage_key,
        )
        .await
        {
            Ok(file) => imported.push(json!({ "id": file.uuid, "name": file.name })),
            Err(e) => {
                let _ = state.storage.delete(&storage_key).await;
                skipped.push(json!({ "name": name, "reason": format!("{e}") }));
            }
        }
    }

    Ok(json!({
        "imported": imported,
        "count": imported.len(),
        "skipped": skipped,
        "truncated": truncated,
        "folder": folder,
        // `name` powers the activity row's target_title.
        "name": format!("{} file(s)", imported.len()),
    }))
}

async fn files_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let deleted = files_repo::delete_file(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    if let Err(e) = state.storage.delete(&deleted.storage_key).await {
        tracing::warn!(error = %e, key = %deleted.storage_key, "mcp files_delete: storage cleanup failed");
    }
    Ok(json!({ "deleted": true, "id": id }))
}

async fn folders_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let name = clean_name(&req_str(args, "name")?)?;
    let parent = opt_trimmed(args, "parent_id");
    if let Some(p) = parent.as_deref() {
        files_repo::get_folder(db, owner, p)
            .await?
            .ok_or_else(|| invalid("parent folder does not exist"))?;
    }
    let folder = files_repo::create_folder(db, owner, &name, parent.as_deref()).await?;
    Ok(json!(FolderDto::from(folder)))
}

// ── AI ────────────────────────────────────────────────────────────────────────

async fn ai_providers(state: &AppState, owner: &str) -> ToolResult {
    let configured: HashSet<String> = ai_repo::list_configured(&state.db, owner)
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

async fn ai_chat(state: &AppState, owner: &str, args: &Value) -> ToolResult {
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
        let ciphertext = ai_repo::get_ciphertext(&state.db, owner, provider.id())
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

// ── Projects ──────────────────────────────────────────────────────────────────

/// Read an optional `project_id` arg, validating it names an existing owned project.
async fn resolve_project_arg(
    state: &AppState,
    owner: &str,
    args: &Value,
) -> Result<Option<String>, ToolError> {
    match opt_trimmed(args, "project_id") {
        Some(pid) => {
            if kn::get_project(&state.db, owner, &pid).await?.is_none() {
                return Err(invalid("project_id does not match an existing project"));
            }
            Ok(Some(pid))
        }
        None => Ok(None),
    }
}

async fn projects_list(state: &AppState, owner: &str) -> ToolResult {
    let projects = kn::list_projects(&state.db, owner).await?;
    let dtos: Vec<ProjectDto> = projects.into_iter().map(Into::into).collect();
    Ok(json!({ "projects": dtos }))
}

async fn projects_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let project = kn::get_project(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    let counts = kn::member_counts(&state.db, owner, &id).await?;
    let members = kn::project_members(&state.db, owner, &id).await?;
    Ok(json!({ "project": ProjectDto::from(project), "counts": counts, "members": members }))
}

async fn projects_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let name = clean_title(&req_str(args, "name")?)?;
    let summary = opt_str(args, "summary").unwrap_or_default();
    if summary.len() > MAX_IDEA_BODY {
        return Err(invalid("summary is too large"));
    }
    let project = kn::create_project(&state.db, owner, &name, &summary).await?;
    Ok(json!(ProjectDto::from(project)))
}

async fn projects_update(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    kn::get_project(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    let name = match opt_str(args, "name") {
        Some(n) => Some(clean_title(&n)?),
        None => None,
    };
    let summary = opt_str(args, "summary");
    let status = match opt_trimmed(args, "status") {
        Some(s) if PROJECT_STATUSES.contains(&s.as_str()) => Some(s),
        Some(_) => {
            return Err(invalid(
                "invalid status (expected one of: active, archived)",
            ))
        }
        None => None,
    };
    let updated = kn::update_project(
        &state.db,
        owner,
        &id,
        name.as_deref(),
        summary.as_deref(),
        status.as_deref(),
    )
    .await?
    .ok_or_else(not_found)?;
    Ok(json!(ProjectDto::from(updated)))
}

async fn projects_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if kn::delete_project(&state.db, owner, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn projects_add_item(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let project_id = req_str(args, "project_id")?;
    let item_type = req_str(args, "item_type")?;
    let item_id = req_str(args, "item_id")?;
    kn::set_membership(&state.db, owner, &item_type, &item_id, Some(&project_id)).await?;
    Ok(json!({ "added": true, "id": project_id, "item_type": item_type, "item_id": item_id }))
}

async fn projects_remove_item(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let item_type = req_str(args, "item_type")?;
    let item_id = req_str(args, "item_id")?;
    kn::set_membership(&state.db, owner, &item_type, &item_id, None).await?;
    Ok(json!({ "removed": true, "id": item_id, "item_type": item_type }))
}

// ── Pages (hosted web pages) ────────────────────────────────────────────────

/// Validate a page `source_format` arg, defaulting to `html`.
fn clean_source_format(args: &Value) -> Result<String, ToolError> {
    match opt_trimmed(args, "source_format") {
        Some(f) if SOURCE_FORMATS.contains(&f.as_str()) => Ok(f),
        Some(_) => Err(invalid(format!(
            "invalid source_format (expected one of: {})",
            SOURCE_FORMATS.join(", ")
        ))),
        None => Ok("html".to_string()),
    }
}

/// Validate a page `visibility` arg against the whitelist.
fn clean_visibility(v: &str) -> Result<String, ToolError> {
    if VISIBILITIES.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(invalid(format!(
            "invalid visibility (expected one of: {})",
            VISIBILITIES.join(", ")
        )))
    }
}

/// Read an optional `folder_id` arg, validating it names an existing owned folder.
async fn resolve_folder_arg(
    state: &AppState,
    owner: &str,
    args: &Value,
) -> Result<Option<String>, ToolError> {
    match opt_trimmed(args, "folder_id") {
        Some(f) => {
            files_repo::get_folder(&state.db, owner, &f)
                .await?
                .ok_or_else(|| invalid("folder_id does not match an existing folder"))?;
            Ok(Some(f))
        }
        None => Ok(None),
    }
}

/// Make a page DTO's `public_url` absolute against the configured public origin.
fn page_dto(state: &AppState, page: crate::pages::model::PageRow) -> PageDto {
    PageDto::from(page).with_origin(state.config.public_page_origin.as_deref())
}

async fn pages_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let folder = opt_trimmed(args, "folder_id");
    let project = opt_trimmed(args, "project_id");
    let tag = opt_trimmed(args, "tag");
    let q = opt_trimmed(args, "q");
    let visibility = match opt_trimmed(args, "visibility") {
        Some(v) => Some(clean_visibility(&v)?),
        None => None,
    };
    let limit = opt_usize(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let offset = opt_usize(args, "offset").unwrap_or(0);

    let pages = pages_repo::list_pages(
        &state.db,
        owner,
        folder.as_deref(),
        visibility.as_deref(),
        project.as_deref(),
        tag.as_deref(),
        q.as_deref(),
        limit,
        offset,
    )
    .await?;
    let origin = state.config.public_page_origin.as_deref();
    let dtos: Vec<PageSummary> = pages
        .into_iter()
        .map(|p| PageSummary::from(p).with_origin(origin))
        .collect();
    Ok(json!({ "pages": dtos }))
}

async fn pages_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let page = pages_repo::get_page(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(page_dto(state, page)))
}

async fn pages_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let title = clean_title(&req_str(args, "title")?)?;
    // Visibility defaults to `draft` so an agent's page is never self-published.
    let visibility = match opt_trimmed(args, "visibility") {
        Some(v) => clean_visibility(&v)?,
        None => "draft".to_string(),
    };

    // `from_document` is the one-way promote bridge: copy a document's already-
    // sanitized HTML body. Otherwise use the supplied body + source format.
    let (raw_body, source_format) = match opt_trimmed(args, "from_document") {
        Some(doc_id) => {
            let doc = doc_repo::get_document(db, owner, &doc_id)
                .await?
                .ok_or_else(|| invalid("from_document does not match an existing document"))?;
            (doc.body, "html".to_string())
        }
        None => (
            opt_str(args, "body").unwrap_or_default(),
            clean_source_format(args)?,
        ),
    };
    if raw_body.len() > MAX_DOC_BODY {
        return Err(invalid("page is too large"));
    }

    let folder_id = resolve_folder_arg(state, owner, args).await?;
    let project_id = resolve_project_arg(state, owner, args).await?;
    let tags = clean_tags(opt_str_array(args, "tags").unwrap_or_default())?;

    let page = pages_repo::create_page(
        db,
        owner,
        &title,
        &raw_body,
        &source_format,
        &visibility,
        folder_id.as_deref(),
        project_id.as_deref(),
        &tags,
    )
    .await?;
    Ok(json!(page_dto(state, page)))
}

async fn pages_update(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let id = req_str(args, "id")?;
    pages_repo::get_page(db, owner, &id)
        .await?
        .ok_or_else(not_found)?;

    let title = match opt_str(args, "title") {
        Some(t) => Some(clean_title(&t)?),
        None => None,
    };
    if let Some(b) = args.get("body").and_then(|v| v.as_str()) {
        if b.len() > MAX_DOC_BODY {
            return Err(invalid("page is too large"));
        }
    }
    let source_format = match opt_trimmed(args, "source_format") {
        Some(_) => Some(clean_source_format(args)?),
        None => None,
    };
    let visibility = match opt_trimmed(args, "visibility") {
        Some(v) => Some(clean_visibility(&v)?),
        None => None,
    };
    // Validate move targets exist before patching.
    let folder_arg = opt_trimmed(args, "folder_id");
    if let Some(f) = folder_arg.as_deref() {
        files_repo::get_folder(db, owner, f)
            .await?
            .ok_or_else(|| invalid("folder_id does not match an existing folder"))?;
    }
    let project_arg = opt_trimmed(args, "project_id");
    if let Some(p) = project_arg.as_deref() {
        if kn::get_project(db, owner, p).await?.is_none() {
            return Err(invalid("project_id does not match an existing project"));
        }
    }

    let tags = match opt_str_array(args, "tags") {
        Some(t) => Some(clean_tags(t)?),
        None => None,
    };

    let patch = PagePatch {
        title: title.as_deref(),
        body: args.get("body").and_then(|v| v.as_str()),
        source_format: source_format.as_deref(),
        slug: None,
        visibility: visibility.as_deref(),
        folder_id: folder_arg.as_deref().map(Some),
        project_id: project_arg.as_deref().map(Some),
        tags: tags.as_deref(),
    };
    let updated = pages_repo::update_page(db, owner, &id, patch)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(page_dto(state, updated)))
}

async fn pages_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if pages_repo::delete_page(&state.db, owner, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn pages_publish(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let visibility = match opt_trimmed(args, "visibility") {
        Some(v) => clean_visibility(&v)?,
        None => "public".to_string(),
    };
    if visibility == "draft" {
        return Err(invalid(
            "publish expects unlisted or public; use pages_unpublish for draft",
        ));
    }
    let patch = PagePatch {
        visibility: Some(&visibility),
        ..PagePatch::default()
    };
    let page = pages_repo::update_page(&state.db, owner, &id, patch)
        .await?
        .ok_or_else(not_found)?;
    let dto = page_dto(state, page);
    Ok(json!({
        "published": true, "id": dto.id, "title": dto.title,
        "visibility": dto.visibility, "url": dto.public_url
    }))
}

async fn pages_unpublish(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let patch = PagePatch {
        visibility: Some("draft"),
        ..PagePatch::default()
    };
    let page = pages_repo::update_page(&state.db, owner, &id, patch)
        .await?
        .ok_or_else(not_found)?;
    let dto = page_dto(state, page);
    Ok(json!({ "unpublished": true, "id": dto.id, "title": dto.title }))
}

// ── Mindmaps (visual idea maps) ─────────────────────────────────────────────

/// Validate a mindmap `source_format` arg, defaulting to `json`.
fn clean_mindmap_format(args: &Value) -> Result<String, ToolError> {
    match opt_trimmed(args, "source_format") {
        Some(f) if MINDMAP_SOURCE_FORMATS.contains(&f.as_str()) => Ok(f),
        Some(_) => Err(invalid(format!(
            "invalid source_format (expected one of: {})",
            MINDMAP_SOURCE_FORMATS.join(", ")
        ))),
        None => Ok("json".to_string()),
    }
}

/// Parse the `graph` arg (a JSON object) into a `Graph`, defaulting to empty.
fn parse_graph_arg(args: &Value) -> Result<Graph, ToolError> {
    match args.get("graph") {
        Some(v) if !v.is_null() => {
            serde_json::from_value(v.clone()).map_err(|e| invalid(format!("invalid graph: {e}")))
        }
        _ => Ok(Graph::default()),
    }
}

async fn mindmaps_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let folder = opt_trimmed(args, "folder_id");
    let project = opt_trimmed(args, "project_id");
    let tag = opt_trimmed(args, "tag");
    let q = opt_trimmed(args, "q");
    let limit = opt_usize(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let offset = opt_usize(args, "offset").unwrap_or(0);
    let rows = mindmap_repo::list_mindmaps(
        &state.db,
        owner,
        folder.as_deref(),
        project.as_deref(),
        tag.as_deref(),
        None,
        q.as_deref(),
        limit,
        offset,
    )
    .await?;
    let dtos: Vec<MindmapSummary> = rows.into_iter().map(Into::into).collect();
    Ok(json!({ "mindmaps": dtos }))
}

async fn mindmaps_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let row = mindmap_repo::get_mindmap(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(MindmapDto::from(row)))
}

async fn mindmaps_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let title = clean_title(&req_str(args, "title")?)?;
    // An `outline` (Markdown) seeds the graph; otherwise a JSON `graph` is used.
    let (graph, source_format) = match opt_str(args, "outline") {
        Some(outline) if !outline.trim().is_empty() => {
            (from_markdown_outline(&outline), "markdown".to_string())
        }
        _ => (parse_graph_arg(args)?, clean_mindmap_format(args)?),
    };
    let folder_id = resolve_folder_arg(state, owner, args).await?;
    let project_id = resolve_project_arg(state, owner, args).await?;
    let tags = clean_tags(opt_str_array(args, "tags").unwrap_or_default())?;
    let review = clean_review(args)?;
    let row = mindmap_repo::create_mindmap(
        db,
        owner,
        &title,
        &graph,
        &source_format,
        folder_id.as_deref(),
        project_id.as_deref(),
        &tags,
        &review,
    )
    .await?;
    Ok(json!(MindmapDto::from(row)))
}

async fn mindmaps_update(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let id = req_str(args, "id")?;
    mindmap_repo::get_mindmap(db, owner, &id)
        .await?
        .ok_or_else(not_found)?;

    let title = match opt_str(args, "title") {
        Some(t) => Some(clean_title(&t)?),
        None => None,
    };
    // outline (Markdown) wins over an explicit graph for the body.
    let (graph, source_format): (Option<Graph>, Option<String>) = match opt_str(args, "outline") {
        Some(o) if !o.trim().is_empty() => (
            Some(from_markdown_outline(&o)),
            Some("markdown".to_string()),
        ),
        _ => match args.get("graph") {
            Some(v) if !v.is_null() => (Some(parse_graph_arg(args)?), None),
            _ => (None, None),
        },
    };
    let review = opt_review(args)?;
    let tags = match opt_str_array(args, "tags") {
        Some(t) => Some(clean_tags(t)?),
        None => None,
    };
    let folder_arg = opt_trimmed(args, "folder_id");
    if let Some(f) = folder_arg.as_deref() {
        files_repo::get_folder(db, owner, f)
            .await?
            .ok_or_else(|| invalid("folder_id does not match an existing folder"))?;
    }
    let project_arg = opt_trimmed(args, "project_id");
    if let Some(p) = project_arg.as_deref() {
        if kn::get_project(db, owner, p).await?.is_none() {
            return Err(invalid("project_id does not match an existing project"));
        }
    }

    let patch = MindmapPatch {
        title: title.as_deref(),
        graph: graph.as_ref(),
        source_format: source_format.as_deref(),
        review: review.as_deref(),
        folder_id: folder_arg.as_deref().map(Some),
        project_id: project_arg.as_deref().map(Some),
        tags: tags.as_deref(),
    };
    let updated = mindmap_repo::update_mindmap(db, owner, &id, patch)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(MindmapDto::from(updated)))
}

async fn mindmaps_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if mindmap_repo::delete_mindmap(&state.db, owner, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn mindmaps_from_project(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let project_id = req_str(args, "project_id")?;
    let project = kn::get_project(db, owner, &project_id)
        .await?
        .ok_or_else(not_found)?;
    let graph = mindmap_repo::seed_from_project(db, owner, &project_id).await?;
    let title = match opt_trimmed(args, "title") {
        Some(t) => clean_title(&t)?,
        None => clean_title(&format!("{} — mindmap", project.name))?,
    };
    let review = clean_review(args)?;
    let row = mindmap_repo::create_mindmap(
        db,
        owner,
        &title,
        &graph,
        "json",
        None,
        Some(&project_id),
        &[],
        &review,
    )
    .await?;
    Ok(json!(MindmapDto::from(row)))
}

// ── Diagrams (draw.io / mxGraph) ────────────────────────────────────────────

/// A preview arg must be a `data:image/*` URI (rendered in `<img>`, never run).
fn clean_preview_arg(args: &Value) -> Result<String, ToolError> {
    match opt_str(args, "preview") {
        Some(p) if p.is_empty() => Ok(p),
        Some(p) if p.starts_with("data:image/") => {
            if p.len() > crate::diagrams::model::MAX_PREVIEW {
                Err(invalid("preview is too large"))
            } else {
                Ok(p)
            }
        }
        Some(_) => Err(invalid("preview must be a data:image/* URI")),
        None => Ok(String::new()),
    }
}

async fn diagrams_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let folder = opt_trimmed(args, "folder_id");
    let project = opt_trimmed(args, "project_id");
    let tag = opt_trimmed(args, "tag");
    let q = opt_trimmed(args, "q");
    let limit = opt_usize(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let offset = opt_usize(args, "offset").unwrap_or(0);
    let rows = diagrams_repo::list_diagrams(
        &state.db,
        owner,
        folder.as_deref(),
        project.as_deref(),
        tag.as_deref(),
        None,
        q.as_deref(),
        limit,
        offset,
    )
    .await?;
    let dtos: Vec<DiagramSummary> = rows.into_iter().map(Into::into).collect();
    Ok(json!({ "diagrams": dtos }))
}

async fn diagrams_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let row = diagrams_repo::get_diagram(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(DiagramDto::from(row)))
}

async fn diagrams_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let title = clean_title(&req_str(args, "title")?)?;
    let xml = opt_str(args, "xml").unwrap_or_default();
    if xml.len() > MAX_DOC_BODY {
        return Err(invalid("diagram is too large"));
    }
    let preview = clean_preview_arg(args)?;
    let folder_id = resolve_folder_arg(state, owner, args).await?;
    let project_id = resolve_project_arg(state, owner, args).await?;
    let tags = clean_tags(opt_str_array(args, "tags").unwrap_or_default())?;
    let review = clean_review(args)?;
    let row = diagrams_repo::create_diagram(
        db,
        owner,
        &title,
        &xml,
        &preview,
        folder_id.as_deref(),
        project_id.as_deref(),
        &tags,
        &review,
    )
    .await?;
    Ok(json!(DiagramDto::from(row)))
}

async fn diagrams_update(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let db = &state.db;
    let id = req_str(args, "id")?;
    diagrams_repo::get_diagram(db, owner, &id)
        .await?
        .ok_or_else(not_found)?;

    let title = match opt_str(args, "title") {
        Some(t) => Some(clean_title(&t)?),
        None => None,
    };
    let xml = match opt_str(args, "xml") {
        Some(x) if x.len() > MAX_DOC_BODY => return Err(invalid("diagram is too large")),
        other => other,
    };
    let preview = match args.get("preview") {
        Some(_) => Some(clean_preview_arg(args)?),
        None => None,
    };
    let review = opt_review(args)?;
    let tags = match opt_str_array(args, "tags") {
        Some(t) => Some(clean_tags(t)?),
        None => None,
    };
    let folder_arg = opt_trimmed(args, "folder_id");
    if let Some(f) = folder_arg.as_deref() {
        files_repo::get_folder(db, owner, f)
            .await?
            .ok_or_else(|| invalid("folder_id does not match an existing folder"))?;
    }
    let project_arg = opt_trimmed(args, "project_id");
    if let Some(p) = project_arg.as_deref() {
        if kn::get_project(db, owner, p).await?.is_none() {
            return Err(invalid("project_id does not match an existing project"));
        }
    }

    let patch = DiagramPatch {
        title: title.as_deref(),
        xml: xml.as_deref(),
        preview: preview.as_deref(),
        review: review.as_deref(),
        folder_id: folder_arg.as_deref().map(Some),
        project_id: project_arg.as_deref().map(Some),
        tags: tags.as_deref(),
    };
    let updated = diagrams_repo::update_diagram(db, owner, &id, patch)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(DiagramDto::from(updated)))
}

async fn diagrams_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if diagrams_repo::delete_diagram(&state.db, owner, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

// ── Superpages ────────────────────────────────────────────────────────────────

fn parse_layout_arg(args: &Value) -> Result<Layout, ToolError> {
    match args.get("blocks") {
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| invalid(e.to_string())),
        None => Ok(Layout::default()),
    }
}

async fn superpages_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let limit = opt_usize(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let offset = opt_usize(args, "offset").unwrap_or(0);
    let rows = superpage_repo::list_superpages(
        &state.db,
        owner,
        opt_trimmed(args, "folder_id").as_deref(),
        opt_trimmed(args, "project_id").as_deref(),
        opt_trimmed(args, "tag").as_deref(),
        opt_trimmed(args, "review").as_deref(),
        opt_trimmed(args, "q").as_deref(),
        limit,
        offset,
    )
    .await?;
    let summaries: Vec<SuperpageSummary> =
        rows.into_iter().map(SuperpageSummary::from_row).collect();
    Ok(json!({ "superpages": summaries }))
}

async fn superpages_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let row = superpage_repo::get_superpage(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(SuperpageDto::from(row)))
}

async fn superpages_create(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let title = clean_title(&req_str(args, "title")?)?;
    let layout = parse_layout_arg(args)?;
    let tags = match opt_str_array(args, "tags") {
        Some(t) => clean_tags(t)?,
        None => Vec::new(),
    };
    let review = clean_review(args)?;
    let row = superpage_repo::create_superpage(
        &state.db,
        owner,
        &title,
        &layout,
        opt_trimmed(args, "folder_id").as_deref(),
        opt_trimmed(args, "project_id").as_deref(),
        &tags,
        &review,
    )
    .await?;
    Ok(json!(SuperpageDto::from(row)))
}

async fn superpages_update(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let title = match opt_trimmed(args, "title") {
        Some(t) => Some(clean_title(&t)?),
        None => None,
    };
    let layout = if args.get("blocks").is_some() {
        Some(parse_layout_arg(args)?)
    } else {
        None
    };
    let review = opt_review(args)?;
    let tags = match opt_str_array(args, "tags") {
        Some(t) => Some(clean_tags(t)?),
        None => None,
    };
    let updated = superpage_repo::update_superpage(
        &state.db,
        owner,
        &id,
        superpage_repo::SuperpagePatch {
            title: title.as_deref(),
            layout: layout.as_ref(),
            review: review.as_deref(),
            folder_id: None,
            project_id: None,
            tags: tags.as_deref(),
        },
    )
    .await?
    .ok_or_else(not_found)?;
    Ok(json!(SuperpageDto::from(updated)))
}

async fn superpages_delete(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    if superpage_repo::delete_superpage(&state.db, owner, &id).await? {
        Ok(json!({ "deleted": true, "id": id }))
    } else {
        Err(not_found())
    }
}

async fn superpages_context(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let row = superpage_repo::get_superpage(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    let layout: Layout = serde_json::from_str(&row.blocks).unwrap_or_default();
    let ctx = context::resolve_context(&state.db, owner, &row.uuid, &row.title, &layout).await?;
    Ok(json!(ctx))
}

async fn superpages_from_project(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let project_id = req_str(args, "project_id")?;
    let project = kn::get_project(&state.db, owner, &project_id)
        .await?
        .ok_or_else(not_found)?;
    let layout = superpage_repo::seed_from_project(&state.db, owner, &project_id).await?;
    let title = match opt_trimmed(args, "title") {
        Some(t) => clean_title(&t)?,
        None => clean_title(&format!("{} — superpage", project.name))?,
    };
    let review = clean_review(args)?;
    let row = superpage_repo::create_superpage(
        &state.db,
        owner,
        &title,
        &layout,
        None,
        Some(&project_id),
        &[],
        &review,
    )
    .await?;
    Ok(json!(SuperpageDto::from(row)))
}

// ── Knowledge links & search ──────────────────────────────────────────────────

async fn knowledge_link(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let st = req_str(args, "src_type")?;
    let si = req_str(args, "src_id")?;
    let dt = req_str(args, "dst_type")?;
    let di = req_str(args, "dst_id")?;
    let rel = opt_trimmed(args, "relation").unwrap_or_default();
    kn::link_items(&state.db, owner, &st, &si, &dt, &di, &rel).await?;
    Ok(json!({
        "linked": true, "id": si, "src_type": st, "src_id": si,
        "dst_type": dt, "dst_id": di, "relation": rel
    }))
}

async fn knowledge_unlink(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let st = req_str(args, "src_type")?;
    let si = req_str(args, "src_id")?;
    let dt = req_str(args, "dst_type")?;
    let di = req_str(args, "dst_id")?;
    kn::unlink_items(&state.db, owner, &st, &si, &dt, &di).await?;
    Ok(json!({
        "unlinked": true, "id": si, "src_type": st, "src_id": si, "dst_type": dt, "dst_id": di
    }))
}

async fn knowledge_backlinks(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let item_type = req_str(args, "item_type")?;
    let item_id = req_str(args, "item_id")?;
    let links = kn::backlinks(&state.db, owner, &item_type, &item_id).await?;
    Ok(json!({ "links": links }))
}

async fn knowledge_search(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let q = req_str(args, "q")?;
    let limit = opt_usize(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let results = kn::search(&state.db, owner, &q, limit).await?;
    Ok(json!(results))
}

async fn knowledge_tags(state: &AppState, owner: &str) -> ToolResult {
    let tags = kn::tag_counts(&state.db, owner).await?;
    Ok(json!({ "tags": tags }))
}

// ── Review queue ──────────────────────────────────────────────────────────────

async fn review_list(state: &AppState, owner: &str) -> ToolResult {
    let queue: ReviewQueue = kn::review_queue(&state.db, owner).await?;
    Ok(json!(queue))
}

// ── Agent (Claude Code CLI) ───────────────────────────────────────────────────

async fn cli_status(state: &AppState, owner: &str) -> ToolResult {
    let cli = &state.config.cli;
    let runner = state.cli_runner.as_ref();
    let kind = runner.kind().to_string();
    let is_mock = kind == "mock";
    let has_stored_key = crate::ai::repo::get_ciphertext(&state.db, owner, ANTHROPIC_PROVIDER)
        .await?
        .is_some();
    let host_key_env = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .is_some_and(|v| !v.trim().is_empty());
    let minimax_configured = cli.minimax_api_key.is_some();

    let (binary_ok, version, binary_detail) = if !cli.enabled {
        (false, None, None)
    } else if is_mock {
        (true, None, None)
    } else {
        let h = runner.health().await;
        (h.binary_ok, h.version, h.detail)
    };

    let (ready, message) = if !cli.enabled {
        (
            false,
            "The agent runner is disabled. Set CLAUDE_CLI_ENABLED=true on the server.".to_string(),
        )
    } else if !binary_ok {
        (
            false,
            format!(
                "`claude` could not be run on the server ({}). Check CLAUDE_BIN and the server PATH.",
                binary_detail.as_deref().unwrap_or("not found")
            ),
        )
    } else if is_mock {
        (
            true,
            "Using the offline mock runner (CLAUDE_BIN=mock).".to_string(),
        )
    } else if has_stored_key {
        (
            true,
            "Ready — Claude Code will use the Anthropic key from your AI settings.".to_string(),
        )
    } else if host_key_env {
        (
            true,
            "Ready — Claude Code will use the server's ANTHROPIC_API_KEY.".to_string(),
        )
    } else {
        (
            true,
            "claude is installed but no Anthropic key was detected — Claude Code relies on a \
             `claude login` session on the server. (MiniMax uses MINIMAX_API_KEY instead.)"
                .to_string(),
        )
    };

    let claude_detail = if !cli.enabled {
        "The agent runner is disabled."
    } else if !binary_ok {
        "claude is not runnable on the server."
    } else if is_mock {
        "Offline mock runner."
    } else if has_stored_key || host_key_env {
        "Anthropic (key configured)."
    } else {
        "Anthropic (uses a `claude login` session if present)."
    };
    let minimax_available = cli.enabled && binary_ok && (is_mock || minimax_configured);
    let minimax_detail = if !cli.enabled {
        "The agent runner is disabled.".to_string()
    } else if !binary_ok {
        "claude is not runnable on the server.".to_string()
    } else if !is_mock && !minimax_configured {
        "Not configured — set MINIMAX_API_KEY on the server.".to_string()
    } else {
        format!(
            "MiniMax via its Anthropic-compatible endpoint ({}).",
            cli.minimax_model
        )
    };

    Ok(json!({
        "enabled": cli.enabled,
        "kind": kind,
        "binary_ok": binary_ok,
        "version": version,
        "has_stored_key": has_stored_key,
        "host_key_env": host_key_env,
        "ready": ready,
        "message": message,
        "providers": [
            {
                "id": "claude_code",
                "label": "Claude Code",
                "available": cli.enabled && binary_ok,
                "detail": claude_detail,
            },
            {
                "id": "minimax",
                "label": cli.minimax_model,
                "available": minimax_available,
                "detail": minimax_detail,
            },
        ],
        "workspace_roots": state.config.workspace_roots,
    }))
}

async fn cli_runs_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let limit = opt_usize(args, "limit")
        .unwrap_or(CLI_DEFAULT_LIMIT)
        .clamp(1, CLI_MAX_LIMIT);
    let offset = opt_usize(args, "offset").unwrap_or(0);
    let runs = cli_repo::list_runs(
        &state.db,
        owner,
        opt_trimmed(args, "project_id").as_deref(),
        opt_trimmed(args, "status").as_deref(),
        limit,
        offset,
    )
    .await?;
    let summaries: Vec<CliRunSummary> = runs.into_iter().map(Into::into).collect();
    Ok(json!({ "runs": summaries }))
}

async fn cli_runs_get(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let run = cli_repo::get_run(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    Ok(json!(CliRunDto::from(run)))
}

async fn cli_run_cancel(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    cli_repo::get_run(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    if state.cli_runs.cancel(&id) {
        Ok(json!({ "cancelled": true, "id": id }))
    } else {
        Err(ToolError::App(AppError::Conflict(
            "run is not active".into(),
        )))
    }
}

// ── Activity ──────────────────────────────────────────────────────────────────

async fn activity_list(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let project_id = opt_trimmed(args, "project_id");
    let agent = opt_trimmed(args, "agent");
    let run_id = opt_trimmed(args, "run_id");
    let since = opt_trimmed(args, "since");
    let limit = opt_usize(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let rows = activity::list(
        &state.db,
        owner,
        &activity::ActivityFilter {
            project_id: project_id.as_deref(),
            agent: agent.as_deref(),
            run_id: run_id.as_deref(),
            since: since.as_deref(),
        },
        limit,
    )
    .await?;
    let dtos: Vec<activity::ActivityDto> = rows.into_iter().map(Into::into).collect();
    Ok(json!({ "activity": dtos }))
}

// ── Publishing & export ───────────────────────────────────────────────────────

/// Minimal HTML escape for titles interpolated into generated markup.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Render bytes to an owner-scoped stored file, mirroring `files_write`'s
/// storage-then-record pattern with rollback. Returns the file DTO as a value.
async fn persist_artifact(
    state: &AppState,
    owner: &str,
    bytes: &[u8],
    filename: &str,
    mime: &str,
) -> Result<Value, ToolError> {
    let storage_key = Uuid::new_v4().to_string();
    state
        .storage
        .put(&storage_key, bytes)
        .await
        .map_err(AppError::from)?;
    match files_repo::create_file(
        &state.db,
        owner,
        &storage_key,
        filename,
        mime,
        bytes.len() as i64,
        None,
        &storage_key,
    )
    .await
    {
        Ok(file) => Ok(json!(FileDto::from(file))),
        Err(e) => {
            let _ = state.storage.delete(&storage_key).await;
            Err(e.into())
        }
    }
}

fn parse_target(args: &Value) -> Result<TargetFormat, ToolError> {
    match opt_trimmed(args, "format") {
        Some(f) => TargetFormat::parse(&f)
            .ok_or_else(|| invalid("unsupported format (html|markdown|pdf|docx)")),
        None => Ok(TargetFormat::Pdf),
    }
}

/// Render a stored document to a self-contained artifact, persist it as an
/// owner-scoped file, and mark the document published. Synchronous — the only
/// slow path is one bounded Chrome/Pandoc call (pdf/docx need those binaries).
async fn documents_publish(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let id = req_str(args, "id")?;
    let target = parse_target(args)?;
    let doc = doc_repo::get_document(&state.db, owner, &id)
        .await?
        .ok_or_else(not_found)?;
    let bytes = convert::export(&doc.body, SourceFormat::Html, target)
        .await
        .map_err(AppError::from)?;
    let filename = format!("{}.{}", sanitize_filename(&doc.title), target.extension());
    let file = persist_artifact(state, owner, &bytes, &filename, target.content_type()).await?;
    // Publishing approves the draft.
    doc_repo::update_document(&state.db, owner, &id, None, None, Some("published"), None).await?;
    Ok(json!({
        "published": true, "id": id, "title": doc.title,
        "format": target.extension(), "file": file
    }))
}

/// Export a whole project — its member documents concatenated under a title page
/// and table of contents — to a single artifact persisted as an owner-scoped file.
async fn collection_export(state: &AppState, owner: &str, args: &Value) -> ToolResult {
    let project_id = req_str(args, "project_id")?;
    let project = kn::get_project(&state.db, owner, &project_id)
        .await?
        .ok_or_else(not_found)?;
    let target = parse_target(args)?;
    let members = kn::project_members(&state.db, owner, &project_id).await?;

    let mut html = format!("<h1>{}</h1>", esc(&project.name));
    if !project.summary.is_empty() {
        html.push_str(&convert::md_to_html(&project.summary));
    }
    if !members.documents.is_empty() {
        html.push_str("<h2>Contents</h2><ul>");
        for m in &members.documents {
            html.push_str(&format!("<li>{}</li>", esc(&m.title)));
        }
        html.push_str("</ul>");
        for m in &members.documents {
            if let Some(doc) = doc_repo::get_document(&state.db, owner, &m.id).await? {
                html.push_str(&format!("<hr><h2>{}</h2>", esc(&doc.title)));
                html.push_str(&doc.body);
            }
        }
    }

    let bytes = convert::export(&html, SourceFormat::Html, target)
        .await
        .map_err(AppError::from)?;
    let name = opt_trimmed(args, "filename").unwrap_or_else(|| project.slug.clone());
    let filename = format!("{}.{}", sanitize_filename(&name), target.extension());
    let file = persist_artifact(state, owner, &bytes, &filename, target.content_type()).await?;
    Ok(json!({
        "exported": true, "id": project_id, "name": project.name,
        "documents": members.documents.len(), "format": target.extension(), "file": file
    }))
}

// ── Workspace (local disk, jailed to WORKSPACE_ROOTS) ────────────────────────
//
// Generic local file management for MCP clients, against the same allow-listed
// roots as the run grant + browse picker. Every path is resolved through
// `crate::workspace` (canonicalize-under-roots), destructive ops refuse the
// roots themselves, and read/write payloads are capped at `MAX_BLOB`. These
// tools are host-level (not owner-scoped): the jail is the configured roots.

/// Max entries returned by one `workspace_list` call.
const MAX_WS_ENTRIES: usize = 500;

fn ws_roots(state: &AppState) -> Result<&[String], ToolError> {
    let roots = state.config.workspace_roots.as_slice();
    if roots.is_empty() {
        return Err(invalid(
            "workspace tools are disabled — set WORKSPACE_ROOTS on the server",
        ));
    }
    Ok(roots)
}

/// Resolve an existing path under the roots (read/list/delete/move-source).
fn ws_existing(state: &AppState, path: &str) -> Result<std::path::PathBuf, ToolError> {
    crate::workspace::resolve_under_roots(ws_roots(state)?, path).map_err(invalid)
}

/// Resolve a possibly-new path under the roots (write/mkdir/destinations).
fn ws_target(state: &AppState, path: &str) -> Result<std::path::PathBuf, ToolError> {
    crate::workspace::resolve_for_create(ws_roots(state)?, path).map_err(invalid)
}

/// Refuse destructive operations on an allow-listed root itself.
fn ws_guard_root(state: &AppState, target: &std::path::Path) -> Result<(), ToolError> {
    if crate::workspace::is_root(&state.config.workspace_roots, target) {
        return Err(invalid("refusing to modify an allow-listed root itself"));
    }
    Ok(())
}

fn unix_secs(t: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    t.ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

fn ws_name(p: &std::path::Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// The common result shape: `id`/`name` feed the central activity logger.
fn ws_entry(p: &std::path::Path) -> Value {
    json!({ "id": p.to_string_lossy(), "path": p.to_string_lossy(), "name": ws_name(p) })
}

fn io_err(action: &str, e: std::io::Error) -> ToolError {
    invalid(format!("{action} failed: {e}"))
}

async fn workspace_roots_tool(state: &AppState) -> ToolResult {
    let roots: Vec<Value> = ws_roots(state)?
        .iter()
        .map(|r| {
            let canon = std::fs::canonicalize(r).ok();
            json!({
                "path": canon.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| r.clone()),
                "exists": canon.is_some(),
            })
        })
        .collect();
    Ok(json!({ "roots": roots }))
}

async fn workspace_list(state: &AppState, args: &Value) -> ToolResult {
    let target = ws_existing(state, &req_str(args, "path")?)?;
    if !target.is_dir() {
        return Err(invalid(format!("not a directory: {}", target.display())));
    }
    let limit = opt_usize(args, "limit")
        .unwrap_or(MAX_WS_ENTRIES)
        .clamp(1, MAX_WS_ENTRIES);

    let mut entries = Vec::new();
    let mut truncated = false;
    let mut rd = tokio::fs::read_dir(&target)
        .await
        .map_err(|e| io_err("list", e))?;
    while let Ok(Some(entry)) = rd.next_entry().await {
        if entries.len() >= limit {
            truncated = true;
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // Don't follow symlinks here: report them as their own kind.
        let meta = tokio::fs::symlink_metadata(entry.path()).await.ok();
        let kind = match &meta {
            Some(m) if m.is_dir() => "dir",
            Some(m) if m.is_symlink() => "symlink",
            _ => "file",
        };
        entries.push(json!({
            "name": name,
            "path": entry.path().to_string_lossy(),
            "kind": kind,
            "size": meta.as_ref().map(|m| m.len()),
            "modified_unix": meta.as_ref().and_then(|m| unix_secs(m.modified())),
        }));
    }
    entries.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .cmp(&b["name"].as_str().unwrap_or_default().to_lowercase())
    });
    Ok(json!({
        "path": target.to_string_lossy(),
        "entries": entries,
        "truncated": truncated,
    }))
}

async fn workspace_info(state: &AppState, args: &Value) -> ToolResult {
    let target = ws_existing(state, &req_str(args, "path")?)?;
    let meta = std::fs::metadata(&target).map_err(|e| io_err("stat", e))?;
    let mut out = json!({
        "path": target.to_string_lossy(),
        "name": ws_name(&target),
        "kind": if meta.is_dir() { "dir" } else { "file" },
        "size": meta.len(),
        "readonly": meta.permissions().readonly(),
        "modified_unix": unix_secs(meta.modified()),
        "created_unix": unix_secs(meta.created()),
    });
    if meta.is_dir() {
        // Directory info: how much is directly inside.
        let (mut files, mut dirs) = (0u64, 0u64);
        if let Ok(rd) = std::fs::read_dir(&target) {
            for entry in rd.flatten() {
                match entry.file_type() {
                    Ok(t) if t.is_dir() => dirs += 1,
                    _ => files += 1,
                }
            }
        }
        out["entries"] = json!({ "files": files, "dirs": dirs });
    }
    Ok(out)
}

async fn workspace_read(state: &AppState, args: &Value) -> ToolResult {
    let target = ws_existing(state, &req_str(args, "path")?)?;
    if !target.is_file() {
        return Err(invalid(format!("not a file: {}", target.display())));
    }
    let meta = std::fs::metadata(&target).map_err(|e| io_err("stat", e))?;
    if meta.len() as usize > MAX_BLOB {
        return Err(invalid(format!(
            "file exceeds the {MAX_BLOB}-byte tool cap ({} bytes)",
            meta.len()
        )));
    }
    let bytes = tokio::fs::read(&target)
        .await
        .map_err(|e| io_err("read", e))?;
    let mut out = ws_entry(&target);
    out["size"] = json!(bytes.len());
    match String::from_utf8(bytes) {
        Ok(text) => {
            out["encoding"] = json!("utf8");
            out["content"] = json!(text);
        }
        Err(e) => {
            out["encoding"] = json!("base64");
            out["content"] = json!(b64::encode(e.as_bytes()));
        }
    }
    Ok(out)
}

async fn workspace_write(state: &AppState, args: &Value) -> ToolResult {
    let target = ws_target(state, &req_str(args, "path")?)?;
    let overwrite = opt_bool(args, "overwrite").unwrap_or(false);
    let existed = target.exists();
    if existed && !overwrite {
        return Err(invalid(format!(
            "already exists (pass overwrite=true to replace): {}",
            target.display()
        )));
    }
    if existed && !target.is_file() {
        return Err(invalid(format!("not a file: {}", target.display())));
    }

    let text = opt_str(args, "content");
    let b64s = opt_str(args, "content_base64");
    let bytes: Vec<u8> = match (text, b64s) {
        (Some(t), None) => t.into_bytes(),
        (None, Some(b)) => {
            b64::decode(&b).map_err(|e| invalid(format!("invalid base64 content: {e}")))?
        }
        (Some(_), Some(_)) => return Err(invalid("pass `content` OR `content_base64`, not both")),
        (None, None) => return Err(invalid("missing `content` (text) or `content_base64`")),
    };
    if bytes.len() > MAX_BLOB {
        return Err(invalid(format!(
            "content exceeds the {MAX_BLOB}-byte tool cap"
        )));
    }
    tokio::fs::write(&target, &bytes)
        .await
        .map_err(|e| io_err("write", e))?;
    let mut out = ws_entry(&target);
    out["size"] = json!(bytes.len());
    out["created"] = json!(!existed);
    Ok(out)
}

async fn workspace_mkdir(state: &AppState, args: &Value) -> ToolResult {
    let target = ws_target(state, &req_str(args, "path")?)?;
    if target.exists() {
        return Err(invalid(format!("already exists: {}", target.display())));
    }
    std::fs::create_dir(&target).map_err(|e| io_err("mkdir", e))?;
    Ok(ws_entry(&target))
}

async fn workspace_delete(state: &AppState, args: &Value) -> ToolResult {
    let target = ws_existing(state, &req_str(args, "path")?)?;
    if !target.is_file() {
        return Err(invalid(format!(
            "not a file (use workspace_rmdir for directories): {}",
            target.display()
        )));
    }
    std::fs::remove_file(&target).map_err(|e| io_err("delete", e))?;
    let mut out = ws_entry(&target);
    out["deleted"] = json!(true);
    Ok(out)
}

async fn workspace_rmdir(state: &AppState, args: &Value) -> ToolResult {
    let target = ws_existing(state, &req_str(args, "path")?)?;
    if !target.is_dir() {
        return Err(invalid(format!("not a directory: {}", target.display())));
    }
    ws_guard_root(state, &target)?;
    let recursive = opt_bool(args, "recursive").unwrap_or(false);
    let res = if recursive {
        std::fs::remove_dir_all(&target)
    } else {
        std::fs::remove_dir(&target)
    };
    res.map_err(|e| {
        if !recursive && e.kind() == std::io::ErrorKind::DirectoryNotEmpty {
            invalid(format!(
                "directory is not empty (pass recursive=true): {}",
                target.display()
            ))
        } else {
            io_err("rmdir", e)
        }
    })?;
    let mut out = ws_entry(&target);
    out["deleted"] = json!(true);
    out["recursive"] = json!(recursive);
    Ok(out)
}

async fn workspace_move(state: &AppState, args: &Value) -> ToolResult {
    let src = ws_existing(state, &req_str(args, "from")?)?;
    ws_guard_root(state, &src)?;
    let dest = ws_target(state, &req_str(args, "to")?)?;
    if dest == src {
        return Err(invalid("`from` and `to` are the same path"));
    }
    let overwrite = opt_bool(args, "overwrite").unwrap_or(false);
    if dest.exists() {
        if !overwrite {
            return Err(invalid(format!(
                "destination already exists (pass overwrite=true): {}",
                dest.display()
            )));
        }
        if dest.is_dir() {
            return Err(invalid(format!(
                "destination is a directory (pass the full target path): {}",
                dest.display()
            )));
        }
    }
    match std::fs::rename(&src, &dest) {
        Ok(()) => {}
        // EXDEV: across filesystems. Fall back to copy+delete for files.
        Err(e) if e.raw_os_error() == Some(18) && src.is_file() => {
            std::fs::copy(&src, &dest).map_err(|e| io_err("copy", e))?;
            std::fs::remove_file(&src).map_err(|e| io_err("remove source", e))?;
        }
        Err(e) if e.raw_os_error() == Some(18) => {
            return Err(invalid(
                "cannot move a directory across filesystems — copy its files instead",
            ));
        }
        Err(e) => return Err(io_err("move", e)),
    }
    let mut out = ws_entry(&dest);
    out["from"] = json!(src.to_string_lossy());
    Ok(out)
}

async fn workspace_copy(state: &AppState, args: &Value) -> ToolResult {
    let src = ws_existing(state, &req_str(args, "from")?)?;
    if !src.is_file() {
        return Err(invalid(format!(
            "only files can be copied (got a directory): {}",
            src.display()
        )));
    }
    let dest = ws_target(state, &req_str(args, "to")?)?;
    if dest == src {
        return Err(invalid("`from` and `to` are the same path"));
    }
    if dest.exists() && !opt_bool(args, "overwrite").unwrap_or(false) {
        return Err(invalid(format!(
            "destination already exists (pass overwrite=true): {}",
            dest.display()
        )));
    }
    let size = std::fs::copy(&src, &dest).map_err(|e| io_err("copy", e))?;
    let mut out = ws_entry(&dest);
    out["from"] = json!(src.to_string_lossy());
    out["size"] = json!(size);
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

fn bool_schema(desc: &str) -> Value {
    json!({ "type": "boolean", "description": desc })
}

/// The full set of tools advertised to MCP clients: the static hand-written
/// catalog plus any tools contributed by enabled plugins (Phase 16; none until
/// the 16.B loader populates the registry).
pub fn definitions(state: &AppState) -> Vec<Value> {
    let mut defs = static_definitions();
    defs.extend(state.plugins.tool_defs());
    defs
}

/// The static, hand-written tool catalog. The drift-guard test pins exactly
/// this list (names ⊆ `known`, equal counts); plugin tools are advertised
/// dynamically via [`definitions`] and never appear here.
fn static_definitions() -> Vec<Value> {
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
            "Create a new idea. Body is Markdown. Agent writes default to review=draft \
             (pending human approval); pass review=published to skip the queue.",
            json!({
                "title": str_schema("idea title (required)"),
                "body": str_schema("Markdown body"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "tags" }),
                "status": str_schema(&status_desc),
                "review": str_schema("draft | published (default draft)"),
                "project_id": str_schema("project to file this idea under (optional)"),
            }),
            &["title"],
        ),
        def(
            "ideas_update",
            "Update fields of an existing idea. Only provided fields change. Pass \
             review=published to approve/publish a draft.",
            json!({
                "id": str_schema("idea id (required)"),
                "title": str_schema("new title"),
                "body": str_schema("new Markdown body"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "replacement tag set" }),
                "status": str_schema(&status_desc),
                "review": str_schema("draft | published"),
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
            "Create an HTML document (body sanitized server-side). Agent writes default to \
             review=draft; pass review=published to skip the review queue.",
            json!({
                "title": str_schema("document title (required)"),
                "body": str_schema("HTML body"),
                "review": str_schema("draft | published (default draft)"),
                "project_id": str_schema("project to file this document under (optional)"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "tags" }),
            }),
            &["title"],
        ),
        def(
            "documents_update",
            "Update a document's title and/or HTML body (sanitized). Pass review=published to \
             approve/publish a draft.",
            json!({
                "id": str_schema("document id (required)"),
                "title": str_schema("new title"),
                "body": str_schema("new HTML body"),
                "review": str_schema("draft | published"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "replacement tag set" }),
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
            "files_import",
            "Import local files into Baitler Files server-side (no base64 needed): give an \
             absolute path to a file or directory within an allow-listed root and Baitler reads \
             the bytes from disk. Great for importing images/documents from a local folder.",
            json!({
                "path": str_schema("absolute local file or directory path (within an allowed root)"),
                "recursive": bool_schema("when path is a directory, descend into subfolders (default true)"),
                "folder": str_schema("destination Baitler folder id; omit for the root"),
            }),
            &["path"],
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
        // Projects
        def(
            "projects_list",
            "List projects (groupings of ideas/documents/files for a piece of work).",
            json!({}),
            &[],
        ),
        def(
            "projects_get",
            "Fetch a project with its member counts (incl. pending drafts) and members by type.",
            json!({ "id": str_schema("project id") }),
            &["id"],
        ),
        def(
            "projects_create",
            "Create a project. A URL-safe slug is derived from the name.",
            json!({
                "name": str_schema("project name (required)"),
                "summary": str_schema("Markdown summary"),
            }),
            &["name"],
        ),
        def(
            "projects_update",
            "Update a project's name, summary, and/or status (active|archived).",
            json!({
                "id": str_schema("project id (required)"),
                "name": str_schema("new name"),
                "summary": str_schema("new summary"),
                "status": str_schema("active | archived"),
            }),
            &["id"],
        ),
        def(
            "projects_delete",
            "Delete a project. Members are detached (their project_id cleared), never deleted.",
            json!({ "id": str_schema("project id") }),
            &["id"],
        ),
        def(
            "projects_add_item",
            "Add an item to a project (sets its project membership).",
            json!({
                "project_id": str_schema("project id"),
                "item_type": str_schema(MEMBER_TYPES_DESC),
                "item_id": str_schema("the item's id"),
            }),
            &["project_id", "item_type", "item_id"],
        ),
        def(
            "projects_remove_item",
            "Remove an item from its project.",
            json!({
                "item_type": str_schema(MEMBER_TYPES_DESC),
                "item_id": str_schema("the item's id"),
            }),
            &["item_type", "item_id"],
        ),
        // Pages (hosted web pages)
        def(
            "pages_list",
            "List hosted web pages (id, title, slug, visibility, public_url, …). Optional filters.",
            json!({
                "folder_id": str_schema("only pages in this folder"),
                "visibility": str_schema("draft | unlisted | public"),
                "project_id": str_schema("only pages in this project"),
                "q": str_schema("case-insensitive search over title and body"),
                "limit": int_schema("max results (default 100, max 500)"),
                "offset": int_schema("pagination offset"),
            }),
            &[],
        ),
        def(
            "pages_get",
            "Fetch a single page (including its sanitized HTML body and public_url) by id.",
            json!({ "id": str_schema("page id") }),
            &["id"],
        ),
        def(
            "pages_create",
            "Create a hosted web page from Markdown or HTML (body sanitized server-side). \
             Visibility defaults to draft (not served) so an agent's page is never \
             self-published — call pages_publish to share it. Pass from_document to promote \
             an existing document's HTML into a new page.",
            json!({
                "title": str_schema("page title (required)"),
                "body": str_schema("page body (Markdown or HTML per source_format)"),
                "source_format": str_schema("html | markdown (default html)"),
                "visibility": str_schema("draft | unlisted | public (default draft)"),
                "folder_id": str_schema("folder to file this page under (optional)"),
                "project_id": str_schema("project to file this page under (optional)"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "tags" }),
                "from_document": str_schema("document id to promote into a new page (optional)"),
            }),
            &["title"],
        ),
        def(
            "pages_update",
            "Update a page's fields (only provided fields change). Body is re-sanitized. \
             folder_id/project_id set membership; visibility folds in here too.",
            json!({
                "id": str_schema("page id (required)"),
                "title": str_schema("new title"),
                "body": str_schema("new body (Markdown or HTML per source_format)"),
                "source_format": str_schema("html | markdown"),
                "visibility": str_schema("draft | unlisted | public"),
                "folder_id": str_schema("move into this folder"),
                "project_id": str_schema("file under this project"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "replacement tag set" }),
            }),
            &["id"],
        ),
        def(
            "pages_delete",
            "Delete a page by id (its cross-type links are scrubbed).",
            json!({ "id": str_schema("page id") }),
            &["id"],
        ),
        def(
            "pages_publish",
            "Publish a page to a served URL and return its shareable public url. \
             visibility unlisted (link-only, noindex) or public (indexable); defaults to public.",
            json!({
                "id": str_schema("page id"),
                "visibility": str_schema("unlisted | public (default public)"),
            }),
            &["id"],
        ),
        def(
            "pages_unpublish",
            "Unpublish a page (→ draft); its URL immediately 404s.",
            json!({ "id": str_schema("page id") }),
            &["id"],
        ),
        // Mindmaps (visual idea maps)
        def(
            "mindmaps_list",
            "List mindmaps (id, title, source_format, review, …). Optional filters.",
            json!({
                "folder_id": str_schema("only mindmaps in this folder"),
                "project_id": str_schema("only mindmaps in this project"),
                "tag": str_schema("only mindmaps carrying this tag"),
                "q": str_schema("case-insensitive search over title and node labels"),
                "limit": int_schema("max results (default 100, max 500)"),
                "offset": int_schema("pagination offset"),
            }),
            &[],
        ),
        def(
            "mindmaps_get",
            "Fetch a single mindmap (including its parsed node/edge graph) by id.",
            json!({ "id": str_schema("mindmap id") }),
            &["id"],
        ),
        def(
            "mindmaps_create",
            "Create a mindmap from a JSON node/edge graph or a Markdown `outline` \
             (headings/bullets → tree). Node labels are plain text. Agent writes default to \
             review=draft; pass review=published to skip the queue.",
            json!({
                "title": str_schema("mindmap title (required)"),
                "graph": json!({ "type": "object", "description": "{ nodes:[{id,label,parent?,x?,y?,color?,item_type?,item_id?}], edges:[{from,to,label?}] }" }),
                "outline": str_schema("Markdown outline to seed the graph (alternative to graph)"),
                "source_format": str_schema("json | markdown (default json)"),
                "review": str_schema("draft | published (default draft)"),
                "folder_id": str_schema("folder to file this mindmap under (optional)"),
                "project_id": str_schema("project to file this mindmap under (optional)"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "tags" }),
            }),
            &["title"],
        ),
        def(
            "mindmaps_update",
            "Update a mindmap (only provided fields change). Supply `graph` or an `outline` to \
             replace the body; pass review=published to approve a draft.",
            json!({
                "id": str_schema("mindmap id (required)"),
                "title": str_schema("new title"),
                "graph": json!({ "type": "object", "description": "replacement node/edge graph" }),
                "outline": str_schema("Markdown outline to rebuild the graph from"),
                "source_format": str_schema("json | markdown"),
                "review": str_schema("draft | published"),
                "folder_id": str_schema("move into this folder"),
                "project_id": str_schema("file under this project"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "replacement tag set" }),
            }),
            &["id"],
        ),
        def(
            "mindmaps_delete",
            "Delete a mindmap by id (its cross-type links are scrubbed).",
            json!({ "id": str_schema("mindmap id") }),
            &["id"],
        ),
        def(
            "mindmaps_from_project",
            "Seed a new mindmap from a project: its ideas become nodes and their cross-links \
             become edges, laid out radially around a central project node.",
            json!({
                "project_id": str_schema("project id (required)"),
                "title": str_schema("title for the new mindmap (optional)"),
                "review": str_schema("draft | published (default draft)"),
            }),
            &["project_id"],
        ),
        // Diagrams (draw.io / mxGraph)
        def(
            "diagrams_list",
            "List draw.io diagrams (id, title, preview, review, …). Optional filters.",
            json!({
                "folder_id": str_schema("only diagrams in this folder"),
                "project_id": str_schema("only diagrams in this project"),
                "tag": str_schema("only diagrams carrying this tag"),
                "q": str_schema("case-insensitive search over title and diagram labels"),
                "limit": int_schema("max results (default 100, max 500)"),
                "offset": int_schema("pagination offset"),
            }),
            &[],
        ),
        def(
            "diagrams_get",
            "Fetch a single diagram (including its mxGraph XML and preview) by id.",
            json!({ "id": str_schema("diagram id") }),
            &["id"],
        ),
        def(
            "diagrams_create",
            "Create a draw.io diagram from mxGraph `xml`, with an optional rendered `preview` \
             (a data:image/* URI). Agent writes default to review=draft.",
            json!({
                "title": str_schema("diagram title (required)"),
                "xml": str_schema("mxGraph XML body"),
                "preview": str_schema("rendered SVG/PNG as a data:image/* URI (optional)"),
                "review": str_schema("draft | published (default draft)"),
                "folder_id": str_schema("folder to file this diagram under (optional)"),
                "project_id": str_schema("project to file this diagram under (optional)"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "tags" }),
            }),
            &["title"],
        ),
        def(
            "diagrams_update",
            "Update a diagram (only provided fields change). Pass review=published to approve a draft.",
            json!({
                "id": str_schema("diagram id (required)"),
                "title": str_schema("new title"),
                "xml": str_schema("new mxGraph XML body"),
                "preview": str_schema("new preview data:image/* URI"),
                "review": str_schema("draft | published"),
                "folder_id": str_schema("move into this folder"),
                "project_id": str_schema("file under this project"),
                "tags": json!({ "type": "array", "items": { "type": "string" }, "description": "replacement tag set" }),
            }),
            &["id"],
        ),
        def(
            "diagrams_delete",
            "Delete a diagram by id (its cross-type links are scrubbed).",
            json!({ "id": str_schema("diagram id") }),
            &["id"],
        ),
        // Superpages (composed canvas)
        def(
            "superpages_list",
            "List superpages (freeform canvases of parts: text, code, image, file, webpage, mindmap, diagram). Optional filters.",
            json!({
                "folder_id": str_schema("only superpages in this folder"),
                "project_id": str_schema("only superpages in this project"),
                "tag": str_schema("only superpages carrying this tag"),
                "q": str_schema("search title and block text"),
                "limit": int_schema("max results (default 100, max 500)"),
                "offset": int_schema("pagination offset"),
            }),
            &[],
        ),
        def(
            "superpages_get",
            "Fetch a superpage by id (layout JSON with block references).",
            json!({ "id": str_schema("superpage id") }),
            &["id"],
        ),
        def(
            "superpages_create",
            "Create a superpage (freeform canvas) from a parts layout. Agent writes default to review=draft.",
            json!({
                "title": str_schema("superpage title (required)"),
                "blocks": json!({
                    "type": "object",
                    "description": "{ layout: canvas, blocks: [{ id, kind, x, y, w, h, … }] }. \
                        Each part has pixel geometry x/y/w/h plus kind-specific fields: \
                        text → { markdown }; code → { text, lang }; \
                        image → { src: upload|url|generated, item_id (upload, a file id) | url, text (caption) }; \
                        file → { item_id (a file id) }; \
                        webpage → { web_kind: url|page, url | item_id (a page id) }; \
                        mindmap → { item_id (a mindmap id) }; diagram → { item_id (a diagram id) }. \
                        Referenced items must already exist and belong to the owner.",
                }),
                "review": str_schema("draft | published (default draft)"),
                "folder_id": str_schema("folder id (optional)"),
                "project_id": str_schema("project id (optional)"),
                "tags": json!({ "type": "array", "items": { "type": "string" } }),
            }),
            &["title"],
        ),
        def(
            "superpages_update",
            "Update a superpage (only provided fields change). Pass review=published to approve.",
            json!({
                "id": str_schema("superpage id (required)"),
                "title": str_schema("new title"),
                "blocks": json!({ "type": "object", "description": "replacement layout" }),
                "review": str_schema("draft | published"),
                "tags": json!({ "type": "array", "items": { "type": "string" } }),
            }),
            &["id"],
        ),
        def(
            "superpages_delete",
            "Delete a superpage by id.",
            json!({ "id": str_schema("superpage id") }),
            &["id"],
        ),
        def(
            "superpages_context",
            "Return a superpage layout with every part resolved (inline text/code, plus titles, \
             bodies, and previews for referenced files/pages/mindmaps/diagrams) for agent grounding in one call.",
            json!({ "id": str_schema("superpage id") }),
            &["id"],
        ),
        def(
            "superpages_from_project",
            "Create a superpage whose embed blocks reference every member of a project.",
            json!({
                "project_id": str_schema("project id (required)"),
                "title": str_schema("title for the new superpage (optional)"),
                "review": str_schema("draft | published (default draft)"),
            }),
            &["project_id"],
        ),
        // Knowledge graph + search
        def(
            "knowledge_link",
            "Create a symmetric cross-type link between two knowledge items.",
            json!({
                "src_type": str_schema(ITEM_TYPES_DESC),
                "src_id": str_schema("source item id"),
                "dst_type": str_schema(ITEM_TYPES_DESC),
                "dst_id": str_schema("target item id"),
                "relation": str_schema("optional label, e.g. contains | implements | references"),
            }),
            &["src_type", "src_id", "dst_type", "dst_id"],
        ),
        def(
            "knowledge_unlink",
            "Remove the cross-type link between two items.",
            json!({
                "src_type": str_schema(ITEM_TYPES_DESC),
                "src_id": str_schema("source item id"),
                "dst_type": str_schema(ITEM_TYPES_DESC),
                "dst_id": str_schema("target item id"),
            }),
            &["src_type", "src_id", "dst_type", "dst_id"],
        ),
        def(
            "knowledge_backlinks",
            "List everything linked to an item, as typed references with titles.",
            json!({
                "item_type": str_schema(ITEM_TYPES_DESC),
                "item_id": str_schema("the item's id"),
            }),
            &["item_type", "item_id"],
        ),
        def(
            "knowledge_search",
            "Full-text search across ideas, documents, projects, files, pages, mindmaps, \
             diagrams, and superpages. Returns typed, ranked sections with highlighted snippets — \
             the agent's entry point for answering questions from the knowledge base.",
            json!({
                "q": str_schema("the search query"),
                "limit": int_schema("max hits per type (default 100, max 500)"),
            }),
            &["q"],
        ),
        def(
            "knowledge_tags",
            "List the cross-type tag taxonomy — every distinct tag across ideas, documents, and \
             pages with how many items carry it (use a tag with the list tools to browse).",
            json!({}),
            &[],
        ),
        // Publishing & export
        def(
            "documents_publish",
            "Render a document to a self-contained artifact (html|markdown|pdf|docx, default pdf), \
             save it as a file, and mark the document published. pdf needs Chrome, docx needs Pandoc.",
            json!({
                "id": str_schema("document id"),
                "format": str_schema("html | markdown | pdf | docx (default pdf)"),
            }),
            &["id"],
        ),
        def(
            "collection_export",
            "Export a whole project (its member documents, with a title page + contents) to one \
             artifact saved as a file.",
            json!({
                "project_id": str_schema("project id"),
                "format": str_schema("html | markdown | pdf | docx (default pdf)"),
                "filename": str_schema("base name for the result file (optional)"),
            }),
            &["project_id"],
        ),
        // Activity / provenance
        def(
            "activity_list",
            "List recent activity (who did what), newest first — the provenance/audit trail.",
            json!({
                "project_id": str_schema("only activity for this project"),
                "agent": str_schema("only activity by this agent label"),
                "run_id": str_schema("only activity from this CLI run"),
                "since": str_schema("ISO-8601 timestamp lower bound"),
                "limit": int_schema("max results (default 100, max 500)"),
            }),
            &[],
        ),
        def(
            "review_list",
            "List ideas and documents pending human approval (review=draft) — the portal Review tab.",
            json!({}),
            &[],
        ),
        // Agent (Claude Code CLI)
        def(
            "cli_status",
            "Agent runner readiness: whether Claude Code is enabled, the binary probe, API keys, \
             and per-provider availability (same signal as the portal Agent page).",
            json!({}),
            &[],
        ),
        def(
            "cli_runs_list",
            "List past Agent (Claude Code CLI) runs, newest first.",
            json!({
                "project_id": str_schema("only runs scoped to this project"),
                "status": str_schema("running | succeeded | failed | cancelled"),
                "limit": int_schema("max results (default 50, max 200)"),
                "offset": int_schema("pagination offset"),
            }),
            &[],
        ),
        def(
            "cli_runs_get",
            "Fetch a single Agent run by id (includes result_text when finished).",
            json!({ "id": str_schema("run id") }),
            &["id"],
        ),
        def(
            "cli_run_cancel",
            "Cancel an in-flight Agent run.",
            json!({ "id": str_schema("run id") }),
            &["id"],
        ),
        // Workspace (local disk, jailed to WORKSPACE_ROOTS)
        def(
            "workspace_roots",
            "List the host directories workspace tools may access (the configured \
             WORKSPACE_ROOTS allow-list). Every workspace path must live under one of these.",
            json!({}),
            &[],
        ),
        def(
            "workspace_list",
            "List a workspace directory: name, kind (file|dir|symlink), size, mtime per entry.",
            json!({
                "path": str_schema("absolute directory path (within a workspace root)"),
                "limit": int_schema("max entries (default and max 500)"),
            }),
            &["path"],
        ),
        def(
            "workspace_info",
            "File or directory metadata: kind, size, mtime/ctime (unix seconds), readonly; \
             directories include direct file/dir counts.",
            json!({ "path": str_schema("absolute path (within a workspace root)") }),
            &["path"],
        ),
        def(
            "workspace_read",
            "Read a workspace file (max 24 MB). UTF-8 files come back as text \
             (encoding=utf8); binary files Base64 (encoding=base64).",
            json!({ "path": str_schema("absolute file path (within a workspace root)") }),
            &["path"],
        ),
        def(
            "workspace_write",
            "Write a workspace file from text or Base64 (max 24 MB). Fails if the file \
             exists unless overwrite=true; the parent directory must already exist.",
            json!({
                "path": str_schema("absolute file path (within a workspace root)"),
                "content": str_schema("text content (UTF-8)"),
                "content_base64": str_schema("binary content, Base64-encoded (alternative to `content`)"),
                "overwrite": bool_schema("replace an existing file (default false)"),
            }),
            &["path"],
        ),
        def(
            "workspace_mkdir",
            "Create one new directory (the parent must already exist).",
            json!({ "path": str_schema("absolute directory path (within a workspace root)") }),
            &["path"],
        ),
        def(
            "workspace_delete",
            "Delete a workspace FILE (directories: use workspace_rmdir).",
            json!({ "path": str_schema("absolute file path (within a workspace root)") }),
            &["path"],
        ),
        def(
            "workspace_rmdir",
            "Delete a workspace directory. Must be empty unless recursive=true. \
             Allow-listed roots themselves are protected.",
            json!({
                "path": str_schema("absolute directory path (within a workspace root)"),
                "recursive": bool_schema("delete contents too (default false)"),
            }),
            &["path"],
        ),
        def(
            "workspace_move",
            "Move or RENAME a file/directory to a new full path (both within the roots). \
             Cross-filesystem moves are supported for files.",
            json!({
                "from": str_schema("absolute source path"),
                "to": str_schema("absolute destination path (the new full path, not a parent dir)"),
                "overwrite": bool_schema("replace an existing destination file (default false)"),
            }),
            &["from", "to"],
        ),
        def(
            "workspace_copy",
            "Copy a workspace FILE to a new full path (both within the roots).",
            json!({
                "from": str_schema("absolute source file path"),
                "to": str_schema("absolute destination path (the new full path, not a parent dir)"),
                "overwrite": bool_schema("replace an existing destination (default false)"),
            }),
            &["from", "to"],
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
        // Scoped to the STATIC catalog: plugin tools (Phase 16) are advertised
        // dynamically through the registry and never need this lockstep edit.
        let advertised: Vec<String> = static_definitions()
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
            "files_import",
            "files_delete",
            "folders_create",
            "projects_list",
            "projects_get",
            "projects_create",
            "projects_update",
            "projects_delete",
            "projects_add_item",
            "projects_remove_item",
            "pages_list",
            "pages_get",
            "pages_create",
            "pages_update",
            "pages_delete",
            "pages_publish",
            "pages_unpublish",
            "mindmaps_list",
            "mindmaps_get",
            "mindmaps_create",
            "mindmaps_update",
            "mindmaps_delete",
            "mindmaps_from_project",
            "diagrams_list",
            "diagrams_get",
            "diagrams_create",
            "diagrams_update",
            "diagrams_delete",
            "superpages_list",
            "superpages_get",
            "superpages_create",
            "superpages_update",
            "superpages_delete",
            "superpages_context",
            "superpages_from_project",
            "knowledge_link",
            "knowledge_unlink",
            "knowledge_backlinks",
            "knowledge_search",
            "knowledge_tags",
            "documents_publish",
            "collection_export",
            "activity_list",
            "review_list",
            "cli_status",
            "cli_runs_list",
            "cli_runs_get",
            "cli_run_cancel",
            "workspace_roots",
            "workspace_list",
            "workspace_info",
            "workspace_read",
            "workspace_write",
            "workspace_mkdir",
            "workspace_delete",
            "workspace_rmdir",
            "workspace_move",
            "workspace_copy",
            "ai_providers",
            "ai_chat",
            "export",
        ];
        for name in &advertised {
            assert!(known.contains(&name.as_str()), "undispatched tool: {name}");
            // The plugin namespace is reserved for registry-dispatched tools;
            // a static tool here would shadow (and collide with) the
            // dynamically-dispatched plugin namespace.
            assert!(
                !name.starts_with(crate::plugins::TOOL_PREFIX),
                "static tool in the plugin namespace: {name}"
            );
        }
        assert_eq!(advertised.len(), known.len(), "tool count drifted");
    }

    #[test]
    fn tool_schemas_are_well_formed() {
        for d in static_definitions() {
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
