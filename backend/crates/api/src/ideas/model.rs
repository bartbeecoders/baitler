//! Data shapes for ideas: DB rows, API DTOs, and request bodies.

use serde::{Deserialize, Serialize};

/// The statuses an idea can have (board columns).
pub const STATUSES: &[&str] = &["inbox", "active", "done", "archived"];

/// The review states gating agent-written content.
pub const REVIEWS: &[&str] = &["draft", "published"];

/// An `idea` row as projected by the repository (timestamps cast to strings).
#[derive(Debug, Clone, Deserialize)]
pub struct IdeaRow {
    pub uuid: String,
    pub owner: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub status: String,
    pub links: Vec<String>,
    /// Draft/published gate (Phase 11): agent writes land as `draft` pending
    /// human approval; everything else is `published`.
    pub review: String,
    /// Project this idea belongs to, if any (Phase 11 membership pointer).
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Public idea representation. `links` are idea uuids.
#[derive(Debug, Serialize)]
pub struct IdeaDto {
    pub id: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub status: String,
    pub links: Vec<String>,
    pub review: String,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<IdeaRow> for IdeaDto {
    fn from(r: IdeaRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            body: r.body,
            tags: r.tags,
            status: r.status,
            links: r.links,
            review: r.review,
            project_id: r.project_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Compact reference to a related idea.
#[derive(Debug, Serialize)]
pub struct IdeaSummary {
    pub id: String,
    pub title: String,
    pub status: String,
}

impl From<IdeaRow> for IdeaSummary {
    fn from(r: IdeaRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            status: r.status,
        }
    }
}

/// `GET /ideas/:id` — the idea plus its resolved related ideas.
#[derive(Debug, Serialize)]
pub struct IdeaDetailDto {
    #[serde(flatten)]
    pub idea: IdeaDto,
    pub related: Vec<IdeaSummary>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIdeaBody {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateIdeaBody {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LinkBody {
    pub target_id: String,
}
