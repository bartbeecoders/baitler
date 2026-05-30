//! Unauthenticated public serving of published pages: `GET /p/{slug}`.
//!
//! This router is wired OUTSIDE the credentialed CORS layer and any (future)
//! auth middleware (see `routes::mod`), and the handler takes **no
//! `CurrentOwner`** — a published page is public, owner-less. The stored `body`
//! is already sanitized on write; this path additionally runs
//! `convert::harden_for_render` (stripping any remote `<img>` so a viewer's
//! browser issues no requests for page content) and emits a no-script CSP. The
//! CSP `sandbox` directive forces each page into an opaque, cookieless origin,
//! so even on a single host a page can't read the app's cookies — though a
//! distinct `PUBLIC_PAGE_ORIGIN` is the durable control once Phase 2 adds them.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::convert;
use crate::state::AppState;

use super::repo;

/// No scripts at all; images only as `data:` URIs; no framing; each page forced
/// into an opaque sandboxed origin. This is the load-bearing control — it holds
/// even if a sanitizer bypass ever lands.
const PUBLIC_CSP: &str = "default-src 'none'; img-src data:; style-src 'unsafe-inline'; \
     base-uri 'none'; form-action 'none'; frame-ancestors 'none'; sandbox";

pub fn router() -> Router<AppState> {
    Router::new().route("/p/{slug}", get(serve))
}

async fn serve(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    match repo::get_served_by_slug(&state.db, &slug).await {
        Ok(Some(page)) => {
            // Harden the FRAGMENT (strips remote image sources); then wrap it in
            // a full document — running ammonia over a `<!doctype>` document
            // would strip `<html>/<head>/<style>`, so the order matters.
            let fragment = convert::harden_for_render(&page.body);
            let doc = format!(
                "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
                 <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
                 <title>{title}</title><style>{css}</style></head><body>{body}</body></html>",
                title = escape(&page.title),
                css = convert::PRINT_CSS,
                body = fragment,
            );
            (
                StatusCode::OK,
                security_headers(&page.visibility),
                Html(doc),
            )
                .into_response()
        }
        // draft / missing / unknown all 404 identically (existence never confirmed).
        Ok(None) => (
            StatusCode::NOT_FOUND,
            security_headers("public"),
            Html(NOT_FOUND_HTML),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, slug = %slug, "public page serve failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "page render failed").into_response()
        }
    }
}

const NOT_FOUND_HTML: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
     <title>Not found</title></head><body><h1>404 — page not found</h1></body></html>";

/// Locked-down response headers for a served page. `unlisted` pages additionally
/// get `noindex` (keep them out of search engines) and `no-store` (don't let an
/// intermediary retain secret-URL content).
fn security_headers(visibility: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PUBLIC_CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    if visibility == "unlisted" {
        h.insert(
            HeaderName::from_static("x-robots-tag"),
            HeaderValue::from_static("noindex"),
        );
        h.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
    }
    h
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
