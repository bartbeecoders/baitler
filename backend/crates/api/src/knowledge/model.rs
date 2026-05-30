//! Data shapes for the knowledge layer: projects, cross-type links, membership.

use serde::{Deserialize, Serialize};

/// Item types that can be a project member or a link endpoint.
pub const ITEM_TYPES: &[&str] = &["idea", "document", "file", "project"];
/// Types that can belong to a project (everything except project itself).
pub const MEMBER_TYPES: &[&str] = &["idea", "document", "file"];
/// Project lifecycle states.
pub const PROJECT_STATUSES: &[&str] = &["active", "archived"];

/// A `project` row as projected by the repository (timestamps → strings).
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectRow {
    pub uuid: String,
    pub owner: String,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Public project representation.
#[derive(Debug, Serialize)]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ProjectRow> for ProjectDto {
    fn from(r: ProjectRow) -> Self {
        Self {
            id: r.uuid,
            name: r.name,
            slug: r.slug,
            summary: r.summary,
            status: r.status,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Per-type membership counts for a project.
#[derive(Debug, Default, Serialize)]
pub struct MemberCounts {
    pub ideas: usize,
    pub documents: usize,
    pub files: usize,
    /// Members still in `review = "draft"` (ideas + documents), i.e. pending approval.
    pub drafts: usize,
}

/// A resolved link endpoint (the *other* end of a `kn_link` edge).
#[derive(Debug, Serialize)]
pub struct LinkRef {
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
    /// Best-effort display title/name (None if the target was since deleted).
    pub title: Option<String>,
    pub relation: String,
}

/// A `kn_link` row (one direction of a symmetric pair).
#[derive(Debug, Clone, Deserialize)]
pub struct LinkRow {
    pub owner: String,
    pub src_type: String,
    pub src_id: String,
    pub dst_type: String,
    pub dst_id: String,
    pub relation: String,
}

/// One project member (idea/document/file), as listed under a project.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemberItem {
    pub id: String,
    pub title: String,
    /// draft/published for ideas & documents; `None` for files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,
}

/// A project's members grouped by type (returned by `projects_get`).
#[derive(Debug, Default, Serialize)]
pub struct ProjectMembers {
    pub ideas: Vec<MemberItem>,
    pub documents: Vec<MemberItem>,
    pub files: Vec<MemberItem>,
}
