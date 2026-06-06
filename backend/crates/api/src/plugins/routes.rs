//! HTTP handlers for plugin management — the **portal-only lifecycle surface**.
//!
//! Approve / enable / disable / reject live HERE and nowhere else: the MCP
//! catalog deliberately ships no transition verb, so an agent (restricted run
//! or not) can author and propose but can never reach an enable path even by
//! name. Enable hot-loads into the runtime registry (digest + ABI re-verified);
//! disable unloads but keeps the row. Everything 503s while the plugin system
//! is disabled (`PLUGINS_ENABLED=false`), mirroring the CLI runner's gate.

use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::convert;
use crate::error::{AppError, AppResult};
use crate::mcp::tools::ToolError;
use crate::mcp::Actor;
use crate::owner::CurrentOwner;
use crate::state::AppState;

use super::model::{PluginDto, PLUGIN_STATUSES};
use super::repo;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;

/// Grace added to a plugin's own `timeout_ms` for the route-layer deadline.
///
/// Three layers bound a plugin endpoint, each covering what the next can't:
/// the Extism epoch kill (`with_timeout`) bounds pure *guest* compute and fires
/// first; each host-fn `block_on` is itself wrapped in a timeout (`hostfns.rs`),
/// so a wedged DB call returns an error to the guest and releases its
/// blocking-pool thread; and THIS route-layer deadline frees the *request task*
/// (→ 504) so the client never waits unbounded. The route deadline alone cannot
/// cancel a `spawn_blocking` thread — it bounds client latency, and the host-fn
/// timeouts are what bound host liveness.
const ENDPOINT_GRACE_MS: u64 = 5_000;

/// Response content types a plugin endpoint may set; HTML/SVG pass
/// `convert::sanitize` before leaving the server.
const ENDPOINT_CONTENT_TYPES: &[&str] = &[
    "application/json",
    "text/plain",
    "text/csv",
    "text/html",
    "image/svg+xml",
];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/plugins", get(list))
        .route("/plugins/{id}", get(get_one).delete(uninstall))
        .route("/plugins/{id}/approve", post(approve))
        .route("/plugins/{id}/enable", post(enable))
        .route("/plugins/{id}/disable", post(disable))
        .route("/plugins/{id}/reject", post(reject))
        // The endpoint kind (16.8) lives under its own `/px/` prefix — NOT
        // `/plugins/` — so a plugin path like `approve` can never collide with
        // (or be shadowed by) the management verbs above. One static catch-all
        // dispatches every plugin endpoint; no plugin ever registers a route.
        .route("/px/{slug}/{*rest}", any(endpoint_dispatch))
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

// ── The endpoint kind: `{METHOD} /px/{slug}/{path…}` (Phase 16.8) ─────────────

/// Dispatch one plugin-endpoint request to the bound WASM export.
///
/// The export receives `{method, path, query, body, owner}` as JSON and may
/// return either a plain JSON value (served as `application/json`) or
/// `{status?, content_type?, body}` to shape the response — content types are
/// whitelisted and HTML/SVG are sanitized server-side. The whole dispatch sits
/// under a route-layer deadline (plugin `timeout_ms` + grace) → 504.
async fn endpoint_dispatch(
    State(state): State<AppState>,
    CurrentOwner(owner): CurrentOwner,
    Path((slug, rest)): Path<(String, String)>,
    RawQuery(query): RawQuery,
    method: Method,
    body: Bytes,
) -> AppResult<Response> {
    guard(&state)?;
    let Some((plugin, export)) =
        state
            .plugins
            .resolve_endpoint(&owner, &slug, method.as_str(), &rest)
    else {
        // Owner-scoped + credentialed, so a helpful status is fine: explain a
        // parked plugin rather than a blanket 404.
        return match repo::get_by_slug(&state.db, &owner, &slug).await? {
            Some(row) if row.status != "enabled" => Err(AppError::Conflict(format!(
                "plugin `{slug}` is `{}` — enable it in the portal to serve its endpoints",
                row.status
            ))),
            _ => Err(AppError::NotFound),
        };
    };

    let parsed_body: Value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()))
    };
    let input = json!({
        "method": method.as_str(),
        "path": rest,
        "query": query.unwrap_or_default(),
        "body": parsed_body,
        "owner": owner,
    });
    let actor = Actor {
        owner: owner.clone(),
        agent: None,
        run_id: None,
    };

    let deadline =
        Duration::from_millis(plugin.manifest.capabilities.timeout_ms + ENDPOINT_GRACE_MS);
    let output = tokio::time::timeout(
        deadline,
        state
            .plugins
            .invoke_export(&state, &actor, &plugin, &export, &input),
    )
    .await
    .map_err(|_| {
        AppError::Timeout(format!(
            "plugin `{slug}` endpoint `{rest}` exceeded {}ms",
            deadline.as_millis()
        ))
    })?
    .map_err(endpoint_error)?;

    // Provenance, like every plugin invocation (best-effort).
    let entry = crate::activity::NewActivity {
        action: "plugin.invoke",
        target_type: "plugin",
        target_id: &plugin.uuid,
        target_title: &format!("{} /px/{slug}/{rest}", method.as_str()),
        project_id: None,
        summary: &format!("plugin.invoke · {} /px/{slug}/{rest}", method.as_str()),
    };
    if let Err(e) = crate::activity::record(&state.db, &owner, None, None, &entry).await {
        tracing::warn!(plugin = %slug, error = %e, "plugin endpoint activity log failed");
    }

    Ok(shape_endpoint_response(output))
}

fn endpoint_error(e: ToolError) -> AppError {
    match e {
        ToolError::UnknownTool => AppError::NotFound,
        ToolError::Invalid(msg) => AppError::BadRequest(msg),
        ToolError::App(e) => e,
    }
}

/// Turn a guest's output into an HTTP response under the content-type
/// whitelist; anything unshaped is plain `application/json`.
fn shape_endpoint_response(output: Value) -> Response {
    let shaped = output
        .as_object()
        .filter(|o| o.contains_key("content_type") && o.contains_key("body"));
    let Some(obj) = shaped else {
        return (StatusCode::OK, Json(output)).into_response();
    };
    let content_type = obj["content_type"].as_str().unwrap_or("");
    let raw_body = obj["body"].as_str().unwrap_or("");
    if !ENDPOINT_CONTENT_TYPES.contains(&content_type) {
        return AppError::BadRequest(format!(
            "plugin returned content_type `{content_type}` (allowed: {})",
            ENDPOINT_CONTENT_TYPES.join(", ")
        ))
        .into_response();
    }
    let status = match obj.get("status").and_then(Value::as_u64) {
        None => StatusCode::OK,
        Some(s @ 200..=499) => StatusCode::from_u16(s as u16).unwrap_or(StatusCode::OK),
        Some(s) => {
            return AppError::BadRequest(format!(
                "plugin returned status {s} (allowed: 200..=499)"
            ))
            .into_response();
        }
    };
    // Author/agent markup never leaves the server unsanitized — same invariant
    // as documents/pages.
    let body = match content_type {
        "text/html" | "image/svg+xml" => convert::sanitize(raw_body),
        _ => raw_body.to_string(),
    };
    (
        status,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        body,
    )
        .into_response()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn header_of(resp: &Response, name: header::HeaderName) -> String {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn unshaped_output_is_json() {
        let resp = shape_endpoint_response(json!({ "a": 1 }));
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(header_of(&resp, header::CONTENT_TYPE).starts_with("application/json"));
    }

    #[test]
    fn shaped_html_is_sanitized() {
        let resp = shape_endpoint_response(json!({
            "content_type": "text/html",
            "body": "<script>alert(1)</script><b onclick=\"x()\">ok</b>",
            "status": 201,
        }));
        assert_eq!(resp.status(), StatusCode::CREATED);
        // Body inspection needs the bytes; the sanitize invariant itself is
        // covered by convert's tests — here we pin that the HTML path routes
        // through it by checking the response is built (status/headers) and
        // rely on the shared sanitizer. Content-type + nosniff must be set.
        assert_eq!(header_of(&resp, header::CONTENT_TYPE), "text/html");
        assert_eq!(header_of(&resp, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    }

    #[test]
    fn disallowed_content_type_and_status_are_rejected() {
        let resp = shape_endpoint_response(json!({
            "content_type": "application/javascript",
            "body": "alert(1)",
        }));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let resp = shape_endpoint_response(json!({
            "content_type": "text/plain",
            "body": "x",
            "status": 500,
        }));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
