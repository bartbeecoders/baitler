//! Opaque-origin serving of plugin UI assets: `GET /plugin-ui/{uuid}/{*path}`.
//!
//! Wired OUTSIDE the credentialed CORS layer (next to `pages::public`, see
//! `routes::mod`) and owner-less: the assets a user enabled are addressed by
//! the plugin's unguessable uuid and served ONLY while `status = "enabled"`.
//! Unlike the zero-JS `/p/{slug}` pages CSP, plugin UI must run its own
//! scripts — the justified step up is `script-src 'self'` — but the document
//! stays **sandboxed without `allow-same-origin`**: an opaque, cookieless
//! origin whose JS can touch neither the app's cookies/storage nor the
//! credentialed API. `connect-src 'none'` forces ALL backend access through
//! the host's postMessage bridge (`PluginFrame`), which relays only to the
//! plugin's own `/px/{slug}/…` endpoints, and `frame-ancestors` pins embedding
//! to the configured app origins.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::Value;

use crate::state::AppState;

use super::repo;

pub fn router() -> Router<AppState> {
    Router::new().route("/plugin-ui/{uuid}/{*path}", get(serve))
}

/// Extension → content type. A closed list: anything else serves as opaque
/// bytes with `nosniff`, so a stray asset can't become an executable surprise.
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "txt" => "text/plain; charset=utf-8",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Bundle-relative asset paths: the same shared rule `plugins_create`
/// validated on write — re-checked here because this is the public boundary.
fn valid_path(path: &str) -> bool {
    super::manifest::valid_asset_path(path)
}

async fn serve(
    State(state): State<AppState>,
    Path((uuid, path)): Path<(String, String)>,
) -> Response {
    if !state.config.plugins.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "plugin system disabled").into_response();
    }
    if !valid_path(&path) {
        return not_found(&state);
    }
    let assets_json = match repo::get_served_ui_assets(&state.db, &uuid).await {
        Ok(Some(json)) if !json.is_empty() => json,
        Ok(_) => return not_found(&state),
        Err(e) => {
            tracing::error!(error = %e, uuid = %uuid, "plugin ui serve failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "asset serve failed").into_response();
        }
    };
    let assets: Value = match serde_json::from_str(&assets_json) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, uuid = %uuid, "stored plugin ui assets unreadable");
            return (StatusCode::INTERNAL_SERVER_ERROR, "asset serve failed").into_response();
        }
    };
    let Some(b64) = assets.get(&path).and_then(Value::as_str) else {
        return not_found(&state);
    };
    let bytes = match crate::mcp::b64::decode(b64) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!(error = %e, uuid = %uuid, path = %path, "stored asset not Base64");
            return (StatusCode::INTERNAL_SERVER_ERROR, "asset serve failed").into_response();
        }
    };
    (
        StatusCode::OK,
        security_headers(&state, content_type(&path)),
        bytes,
    )
        .into_response()
}

/// Missing/draft/disabled/unknown all 404 identically (existence never
/// confirmed), with the same locked-down headers.
fn not_found(state: &AppState) -> Response {
    (
        StatusCode::NOT_FOUND,
        security_headers(state, "text/plain; charset=utf-8"),
        "not found",
    )
        .into_response()
}

/// The plugin-UI CSP. `sandbox allow-scripts` (NO `allow-same-origin`) forces
/// an opaque, cookieless origin; `'self'` in source lists matches the serving
/// URL's origin, so bundle-relative scripts/styles/images still load.
fn security_headers(state: &AppState, content_type: &str) -> HeaderMap {
    let ancestors = if state.config.cors_allowed_origins.is_empty() {
        "'none'".to_string()
    } else {
        state.config.cors_allowed_origins.join(" ")
    };
    let csp = format!(
        "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
         img-src 'self' data:; font-src 'self'; connect-src 'none'; base-uri 'none'; \
         form-action 'none'; frame-ancestors {ancestors}; sandbox allow-scripts"
    );
    let mut h = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&csp) {
        h.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    if let Ok(v) = HeaderValue::from_str(content_type) {
        h.insert(header::CONTENT_TYPE, v);
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_validation_blocks_traversal_and_junk() {
        assert!(valid_path("ui/index.html"));
        assert!(valid_path("ui/assets/app-v1.2.js"));
        assert!(!valid_path(""));
        assert!(!valid_path("/ui/index.html"));
        assert!(!valid_path("ui/../secret"));
        assert!(!valid_path("ui/./x"));
        assert!(!valid_path("ui//x"));
        assert!(!valid_path("ui/sp ace.html"));
    }

    #[test]
    fn content_types_are_a_closed_list() {
        assert_eq!(content_type("ui/index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("a/b.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type("x.svg"), "image/svg+xml");
        assert_eq!(content_type("weird.wasm"), "application/octet-stream");
        assert_eq!(content_type("noext"), "application/octet-stream");
    }
}
