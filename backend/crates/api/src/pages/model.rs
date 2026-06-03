//! Data shapes for hosted web pages (Phase 12).

use serde::{Deserialize, Serialize};

/// Accepted authoring source formats.
pub const SOURCE_FORMATS: &[&str] = &["html", "markdown"];
/// Page visibility states. `draft` is not served; `unlisted`/`public` are.
pub const VISIBILITIES: &[&str] = &["draft", "unlisted", "public"];

/// The public path a page is served at, once published. Empty while `draft`.
/// The absolute `PUBLIC_PAGE_ORIGIN` prefix is applied at the route layer (12.6);
/// here we carry the origin-relative path.
pub fn public_path(slug: &str, visibility: &str) -> String {
    if visibility == "draft" {
        String::new()
    } else {
        format!("/p/{slug}")
    }
}

/// A `page` row as projected by the repository (timestamps → strings).
#[derive(Debug, Clone, Deserialize)]
pub struct PageRow {
    pub uuid: String,
    pub owner: String,
    pub title: String,
    pub body: String,
    pub slug: String,
    pub visibility: String,
    pub source_format: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub version: i64,
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Full page representation (includes the sanitized HTML `body`).
#[derive(Debug, Serialize)]
pub struct PageDto {
    pub id: String,
    pub title: String,
    pub body: String,
    pub slug: String,
    pub visibility: String,
    pub source_format: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub version: i64,
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    /// Origin-relative public URL (`/p/{slug}`), empty while `draft`.
    pub public_url: String,
    pub created_at: String,
    pub updated_at: String,
}

impl PageDto {
    /// Prefix the (origin-relative) `public_url` with the configured public
    /// origin, when one is set and the page is published. No-op for drafts.
    pub fn with_origin(mut self, origin: Option<&str>) -> Self {
        self.public_url = absolutize(self.public_url, origin);
        self
    }
}

impl PageSummary {
    pub fn with_origin(mut self, origin: Option<&str>) -> Self {
        self.public_url = absolutize(self.public_url, origin);
        self
    }
}

fn absolutize(relative: String, origin: Option<&str>) -> String {
    match origin {
        Some(o) if !relative.is_empty() => format!("{o}{relative}"),
        _ => relative,
    }
}

impl From<PageRow> for PageDto {
    fn from(r: PageRow) -> Self {
        let public_url = public_path(&r.slug, &r.visibility);
        Self {
            id: r.uuid,
            title: r.title,
            body: r.body,
            slug: r.slug,
            visibility: r.visibility,
            source_format: r.source_format,
            folder_id: r.folder_id,
            project_id: r.project_id,
            version: r.version,
            published_at: r.published_at,
            tags: r.tags,
            public_url,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Lightweight list entry (omits `body` for payload size).
#[derive(Debug, Serialize)]
pub struct PageSummary {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub visibility: String,
    pub source_format: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub version: i64,
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    pub public_url: String,
    pub updated_at: String,
}

impl From<PageRow> for PageSummary {
    fn from(r: PageRow) -> Self {
        let public_url = public_path(&r.slug, &r.visibility);
        Self {
            id: r.uuid,
            title: r.title,
            slug: r.slug,
            visibility: r.visibility,
            source_format: r.source_format,
            folder_id: r.folder_id,
            project_id: r.project_id,
            version: r.version,
            published_at: r.published_at,
            tags: r.tags,
            public_url,
            updated_at: r.updated_at,
        }
    }
}
