//! Data shapes for files and folders: DB rows, API DTOs, and request bodies.

use serde::{Deserialize, Serialize};

/// A `file` row as projected by the repository (timestamps cast to strings).
#[derive(Debug, Clone, Deserialize)]
pub struct FileRow {
    pub uuid: String,
    pub owner: String,
    pub name: String,
    pub mime: String,
    pub size: i64,
    pub folder_id: Option<String>,
    pub storage_key: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A `folder` row as projected by the repository.
#[derive(Debug, Clone, Deserialize)]
pub struct FolderRow {
    pub uuid: String,
    pub owner: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Public file representation (no owner/storage_key).
#[derive(Debug, Serialize)]
pub struct FileDto {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: i64,
    pub folder_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<FileRow> for FileDto {
    fn from(r: FileRow) -> Self {
        Self {
            id: r.uuid,
            name: r.name,
            mime: r.mime,
            size: r.size,
            folder_id: r.folder_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Public folder representation.
#[derive(Debug, Serialize)]
pub struct FolderDto {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<FolderRow> for FolderDto {
    fn from(r: FolderRow) -> Self {
        Self {
            id: r.uuid,
            name: r.name,
            parent_id: r.parent_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Response for a directory listing.
#[derive(Debug, Serialize)]
pub struct ListingDto {
    /// The folder being listed (`None` at the root).
    pub folder: Option<FolderDto>,
    /// Path from the root to the current folder (inclusive).
    pub breadcrumbs: Vec<FolderDto>,
    pub folders: Vec<FolderDto>,
    pub files: Vec<FileDto>,
}

/// `POST /folders` body.
#[derive(Debug, Deserialize)]
pub struct CreateFolderBody {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// `PATCH /folders/:id` / `PATCH /files/:id` body.
///
/// `folder_id`/`parent_id` use a "double option" so the JSON distinguishes:
/// absent = leave unchanged, `null` = move to root, value = move into that folder.
#[derive(Debug, Default, Deserialize)]
pub struct UpdateFileBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub folder_id: Option<Option<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateFolderBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<String>>,
}

/// Deserialize a present field (including explicit `null`) to `Some(inner)`,
/// leaving an absent field as `None`.
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}
