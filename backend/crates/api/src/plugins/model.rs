//! Data shapes for plugins (Phase 16).

use serde::{Deserialize, Serialize};

/// Operational lifecycle states (`review` is the separate human gate).
pub const PLUGIN_STATUSES: &[&str] = &["draft", "approved", "enabled", "disabled", "rejected"];

/// A `plugin` row as projected by the repository — WITHOUT `wasm_b64` (which
/// can be MBs and is fetched separately for registry load / export).
#[derive(Debug, Clone, Deserialize)]
pub struct PluginRow {
    pub uuid: String,
    pub owner: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub version: String,
    pub version_int: i64,
    pub api_version: i64,
    /// The validated manifest JSON body (see `plugins::manifest::Manifest`).
    pub manifest: String,
    pub kinds: Vec<String>,
    pub status: String,
    pub review: String,
    pub bundle_sha256: String,
    pub project_id: Option<String>,
    pub author_agent: Option<String>,
    pub run_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Public plugin representation (REST + MCP). The manifest comes back as the
/// parsed JSON value, not the raw string, so clients/agents read it directly.
#[derive(Debug, Serialize)]
pub struct PluginDto {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub version: String,
    pub version_int: i64,
    pub api_version: i64,
    pub manifest: serde_json::Value,
    pub kinds: Vec<String>,
    pub status: String,
    pub review: String,
    pub bundle_sha256: String,
    pub project_id: Option<String>,
    pub author_agent: Option<String>,
    pub run_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<PluginRow> for PluginDto {
    fn from(r: PluginRow) -> Self {
        let manifest = serde_json::from_str(&r.manifest).unwrap_or(serde_json::Value::Null);
        Self {
            id: r.uuid,
            name: r.name,
            slug: r.slug,
            description: r.description,
            version: r.version,
            version_int: r.version_int,
            api_version: r.api_version,
            manifest,
            kinds: r.kinds,
            status: r.status,
            review: r.review,
            bundle_sha256: r.bundle_sha256,
            project_id: r.project_id,
            author_agent: r.author_agent,
            run_id: r.run_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}
