//! Built-in MCP (Model Context Protocol) server.
//!
//! Baitler speaks MCP over the **Streamable HTTP** transport at a single
//! endpoint, `POST /mcp`. Clients that speak MCP natively (Claude Code, the
//! Hermes agent, other MCP tools) connect directly; stdio-only clients can use
//! the `baitler-mcp` bridge binary, which forwards stdio JSON-RPC here.
//!
//! The transport here implements the JSON-response variant of Streamable HTTP:
//! a POST carrying one JSON-RPC *request* gets a single JSON response; a POST
//! carrying only *notifications/responses* gets `202 Accepted` with no body.
//! Batched arrays are supported. We do not open server-initiated SSE streams,
//! so `GET`/`DELETE /mcp` return `405` (the client falls back to request/reply).
//!
//! Tools live in [`tools`]; the JSON-RPC envelope in [`protocol`].

mod b64;
mod protocol;
pub mod tools;

use axum::body::Bytes;
use axum::http::header::{ALLOW, AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::config::McpConfig;
use crate::state::AppState;

use tools::ToolError;

/// Server identity reported in `initialize`.
const SERVER_NAME: &str = "baitler";

/// Build the `/mcp` router. When MCP is disabled, this is an empty router (the
/// endpoint simply doesn't exist, and unmatched `/mcp` falls through to the
/// app's standard 404).
pub fn router(cfg: &McpConfig) -> Router<AppState> {
    if !cfg.enabled {
        tracing::info!("MCP server disabled (MCP_ENABLED=false)");
        return Router::new();
    }
    if cfg.auth_token.is_some() {
        tracing::info!("MCP server enabled at POST /mcp (bearer auth required)");
    } else {
        tracing::info!(
            "MCP server enabled at POST /mcp (no auth — set MCP_AUTH_TOKEN if exposed beyond localhost)"
        );
    }
    Router::new().route(
        "/mcp",
        post(handle)
            .get(method_not_allowed)
            .delete(method_not_allowed),
    )
}

/// Handle a Streamable-HTTP POST: authenticate, parse, dispatch each message,
/// and return either the JSON-RPC response(s) or `202 Accepted`.
async fn handle(
    state: axum::extract::State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let state = state.0;

    // Bearer-token gate (only when a token is configured).
    if let Some(expected) = state.config.mcp.auth_token.as_deref() {
        if !authorized(&headers, expected) {
            return unauthorized();
        }
    }

    let value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_response(protocol::error(
                Value::Null,
                protocol::PARSE_ERROR,
                &format!("invalid JSON: {e}"),
            ));
        }
    };

    match value {
        Value::Array(items) => {
            if items.is_empty() {
                return json_response(protocol::error(
                    Value::Null,
                    protocol::INVALID_REQUEST,
                    "empty batch",
                ));
            }
            let mut responses = Vec::new();
            for item in items {
                if let Some(resp) = dispatch(&state, item).await {
                    responses.push(resp);
                }
            }
            if responses.is_empty() {
                accepted()
            } else {
                json_response(Value::Array(responses))
            }
        }
        single => match dispatch(&state, single).await {
            Some(resp) => json_response(resp),
            None => accepted(),
        },
    }
}

/// Dispatch one JSON-RPC message. Returns `Some(response)` for requests and
/// `None` for notifications (which get no reply).
async fn dispatch(state: &AppState, msg: Value) -> Option<Value> {
    let obj = match msg.as_object() {
        Some(o) => o,
        // A non-object message can't carry an id; nothing to respond to.
        None => return None,
    };

    let id = obj.get("id").cloned();
    let method = obj.get("method").and_then(|m| m.as_str());

    let Some(method) = method else {
        // Missing/invalid method: only answer if it was a request (has id).
        return id.map(|id| protocol::error(id, protocol::INVALID_REQUEST, "missing `method`"));
    };

    // No id ⇒ notification ⇒ no response. We accept (ignore) known ones like
    // `notifications/initialized` and `notifications/cancelled`.
    let id = id?;

    let params = obj.get("params").cloned().unwrap_or(Value::Null);

    // tools/call has its own error mapping (tool errors → isError results).
    if method == "tools/call" {
        return Some(handle_tools_call(state, id, &params).await);
    }

    let result: Result<Value, (i64, String)> = match method {
        "initialize" => Ok(initialize_result(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools::definitions() })),
        "resources/list" => Ok(json!({ "resources": [] })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        "logging/setLevel" => Ok(json!({})),
        other => Err((
            protocol::METHOD_NOT_FOUND,
            format!("method not found: {other}"),
        )),
    };

    Some(match result {
        Ok(value) => protocol::success(id, value),
        Err((code, message)) => protocol::error(id, code, &message),
    })
}

/// Build the `initialize` result, echoing the client's requested protocol
/// version when present (our core is version-agnostic).
fn initialize_result(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(protocol::LATEST_PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Baitler personal-assistant API as MCP tools: manage ideas, HTML \
                         documents, files/folders, convert & export content, and run LLM chat \
                         through configured providers. Call tools/list to see all tools.",
    })
}

/// Execute a `tools/call`, mapping tool outcomes to MCP result semantics:
/// success → `content` text; tool/validation errors → `isError: true` result;
/// unknown tool → JSON-RPC method-not-found.
async fn handle_tools_call(state: &AppState, id: Value, params: &Value) -> Value {
    let Some(name) = params.get("name").and_then(|v| v.as_str()) else {
        return protocol::error(id, protocol::INVALID_PARAMS, "missing tool `name`");
    };
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match tools::call(state, name, &args).await {
        Ok(value) => protocol::success(id, tool_ok(value)),
        Err(ToolError::UnknownTool) => protocol::error(
            id,
            protocol::INVALID_PARAMS,
            &format!("unknown tool: {name}"),
        ),
        Err(ToolError::Invalid(msg)) => protocol::success(id, tool_err(&msg)),
        Err(ToolError::App(e)) => {
            tracing::warn!(tool = name, error = %e, "mcp tool failed");
            protocol::success(id, tool_err(&e.to_string()))
        }
    }
}

/// A successful tool result: the value as pretty JSON text content.
fn tool_ok(value: Value) -> Value {
    let text = match &value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": false,
    })
}

/// A tool error result the model can read and react to.
fn tool_err(message: &str) -> Value {
    json!({
        "content": [ { "type": "text", "text": message } ],
        "isError": true,
    })
}

// ── Auth & HTTP response helpers ──────────────────────────────────────────────

/// Check `Authorization: Bearer <token>` against the configured token using a
/// constant-time comparison.
fn authorized(headers: &HeaderMap, expected: &str) -> bool {
    let Some(value) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let presented = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim);
    matches!(presented, Some(p) if constant_time_eq(p.as_bytes(), expected.as_bytes()))
}

/// Length-aware constant-time byte comparison (avoids early-exit timing leaks on
/// the token contents; the length itself is not treated as secret).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn json_response(value: Value) -> Response {
    (StatusCode::OK, Json(value)).into_response()
}

/// Streamable-HTTP: notifications/responses are acknowledged with `202` + no body.
fn accepted() -> Response {
    StatusCode::ACCEPTED.into_response()
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(WWW_AUTHENTICATE, "Bearer")],
        Json(json!({
            "error": { "code": "unauthorized", "message": "missing or invalid bearer token" }
        })),
    )
        .into_response()
}

async fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(ALLOW, "POST")],
        Json(json!({
            "error": {
                "code": "method_not_allowed",
                "message": "MCP is served over POST /mcp (Streamable HTTP, JSON responses)"
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_echoes_requested_protocol_version() {
        let r = initialize_result(&json!({ "protocolVersion": "2024-11-05" }));
        assert_eq!(r["protocolVersion"], "2024-11-05");
        assert_eq!(r["serverInfo"]["name"], SERVER_NAME);
        assert!(r["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_falls_back_to_latest() {
        let r = initialize_result(&json!({}));
        assert_eq!(r["protocolVersion"], protocol::LATEST_PROTOCOL_VERSION);
    }

    #[test]
    fn tool_ok_wraps_value_as_text_content() {
        let r = tool_ok(json!({ "a": 1 }));
        assert_eq!(r["isError"], false);
        assert_eq!(r["content"][0]["type"], "text");
        assert!(r["content"][0]["text"].as_str().unwrap().contains("\"a\""));
    }

    #[test]
    fn tool_err_sets_is_error() {
        let r = tool_err("nope");
        assert_eq!(r["isError"], true);
        assert_eq!(r["content"][0]["text"], "nope");
    }

    #[test]
    fn bearer_auth_constant_time_and_prefix() {
        let mut headers = HeaderMap::new();
        assert!(!authorized(&headers, "secret"));
        headers.insert(AUTHORIZATION, "Bearer secret".parse().unwrap());
        assert!(authorized(&headers, "secret"));
        headers.insert(AUTHORIZATION, "Bearer wrong".parse().unwrap());
        assert!(!authorized(&headers, "secret"));
        headers.insert(AUTHORIZATION, "Basic secret".parse().unwrap());
        assert!(!authorized(&headers, "secret"));
    }

    #[test]
    fn constant_time_eq_basics() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
