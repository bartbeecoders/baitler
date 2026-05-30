//! Data shapes for documents.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentRow {
    pub uuid: String,
    pub owner: String,
    pub title: String,
    pub body: String,
    pub version: i64,
    /// Draft/published gate (Phase 11). See `ideas::model::REVIEWS`.
    pub review: String,
    /// Project this document belongs to, if any (Phase 11 membership pointer).
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct DocumentDto {
    pub id: String,
    pub title: String,
    pub body: String,
    pub version: i64,
    pub review: String,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<DocumentRow> for DocumentDto {
    fn from(r: DocumentRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            body: r.body,
            version: r.version,
            review: r.review,
            project_id: r.project_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Lightweight entry for the document list.
#[derive(Debug, Serialize)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub version: i64,
    pub review: String,
    pub project_id: Option<String>,
    pub updated_at: String,
}

impl From<DocumentRow> for DocumentSummary {
    fn from(r: DocumentRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            version: r.version,
            review: r.review,
            project_id: r.project_id,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateDocumentBody {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateDocumentBody {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    /// draft|published — lets the portal approve/publish a draft.
    #[serde(default)]
    pub review: Option<String>,
}

/// `POST /export` — the shared conversion pathway, usable by any feature.
#[derive(Debug, Deserialize)]
pub struct ExportBody {
    pub content: String,
    /// `html` or `markdown`.
    pub source: String,
    /// `html` | `markdown` | `pdf` | `docx`.
    pub target: String,
    #[serde(default)]
    pub filename: Option<String>,
}
