//! HTTP handlers for plugin management — the **portal-only lifecycle surface**.
//!
//! Approve / enable / disable / reject live HERE and nowhere else: the MCP
//! catalog deliberately ships no transition verb, so an agent (restricted run
//! or not) can author and propose but can never reach an enable path even by
//! name. Enable hot-loads into the runtime registry (digest + ABI re-verified);
//! disable unloads but keeps the row. Everything 503s while the plugin system
//! is disabled (`PLUGINS_ENABLED=false`), mirroring the CLI runner's gate.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::model::{PluginDto, PLUGIN_STATUSES};
use super::repo;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/plugins", get(list))
        .route("/plugins/{id}", get(get_one).delete(uninstall))
        .route("/plugins/{id}/approve", post(approve))
        .route("/plugins/{id}/enable", post(enable))
        .route("/plugins/{id}/disable", post(disable))
        .route("/plugins/{id}/reject", post(reject))
}

fn guard(state: &AppState) -> AppResult<()> {
    if state.config.plugins.enabled {
        Ok(())
    } else {
        Err(AppError::Unavailable(
            "the plugin system is disabled (set PLUGINS_ENABLED=true)".into(),
        ))
    }
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    status: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Serialize)]
struct PluginListResponse {
    plugins: Vec<PluginDto>,
}

/// `GET /plugins` — the management list (status chips, provenance).
async fn list(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<PluginListResponse>> {
    guard(&state)?;
    if let Some(s) = query.status.as_deref() {
        if !PLUGIN_STATUSES.contains(&s) {
            return Err(AppError::BadRequest(format!(
                "invalid status `{s}` (expected one of: {})",
                PLUGIN_STATUSES.join(", ")
            )));
        }
    }
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let rows = repo::list_plugins(
        &state.db,
        &owner,
        query.status.as_deref(),
        query.q.as_deref(),
        limit,
        query.offset.unwrap_or(0),
    )
    .await?;
    Ok(Json(PluginListResponse {
        plugins: rows.into_iter().map(PluginDto::from).collect(),
    }))
}

/// `GET /plugins/{id}` — one plugin (manifest included, WASM never).
async fn get_one(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<PluginDto>> {
    guard(&state)?;
    let row = repo::get_plugin(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(PluginDto::from(row)))
}

/// `POST /plugins/{id}/approve` — the human gate: draft → approved (review
/// flips to published). Does NOT load anything; enable is the separate step.
async fn approve(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<PluginDto>> {
    guard(&state)?;
    let row = repo::set_status(&state.db, &owner, &id, "approved").await?;
    Ok(Json(PluginDto::from(row)))
}

/// `POST /plugins/{id}/enable` — approved/disabled → enabled + hot-load into
/// the registry (sha256 + ABI re-verified). A load failure rolls the status
/// back to disabled so the row never claims to be running when it isn't.
async fn enable(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<PluginDto>> {
    guard(&state)?;
    let row = repo::set_status(&state.db, &owner, &id, "enabled").await?;
    if let Err(e) = state.plugins.load(&state.db, &row).await {
        let _ = repo::set_status(&state.db, &owner, &id, "disabled").await;
        return Err(e);
    }
    Ok(Json(PluginDto::from(row)))
}

/// `POST /plugins/{id}/disable` — unload but keep the row (cheap re-enable).
async fn disable(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<PluginDto>> {
    guard(&state)?;
    let row = repo::set_status(&state.db, &owner, &id, "disabled").await?;
    state.plugins.unload(&owner, &row.slug);
    Ok(Json(PluginDto::from(row)))
}

/// `POST /plugins/{id}/reject` — terminal park (kept for audit until deleted).
async fn reject(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<PluginDto>> {
    guard(&state)?;
    let row = repo::set_status(&state.db, &owner, &id, "rejected").await?;
    state.plugins.unload(&owner, &row.slug);
    Ok(Json(PluginDto::from(row)))
}

#[derive(Debug, Serialize)]
struct UninstallResponse {
    id: String,
    slug: String,
    deleted: bool,
}

/// `DELETE /plugins/{id}` — uninstall: scrub links, cascade kv, drop, unload.
async fn uninstall(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path(id): Path<String>,
) -> AppResult<Json<UninstallResponse>> {
    guard(&state)?;
    let row = repo::delete_plugin(&state.db, &owner, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    state.plugins.unload(&owner, &row.slug);
    Ok(Json(UninstallResponse {
        id: row.uuid,
        slug: row.slug,
        deleted: true,
    }))
}
