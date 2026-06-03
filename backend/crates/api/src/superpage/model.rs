//! Data shapes for superpages (Phase 15).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Block kinds supported in v1.
pub const BLOCK_KINDS: &[&str] = &["embed", "note", "heading"];
/// Item types an embed block may reference (not project/superpage in v1).
pub const EMBED_ITEM_TYPES: &[&str] =
    &["idea", "document", "file", "page", "mindmap", "diagram"];
pub const MAX_BLOCKS: usize = 50;
pub const MAX_NOTE: usize = 50_000;
pub const MAX_HEADING: usize = 500;
pub const MAX_BLOCK_ID_LEN: usize = 64;

/// The canonical superpage body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default = "default_layout_kind")]
    pub layout: String,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

fn default_layout_kind() -> String {
    "grid".to_string()
}

/// One block on the canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Structurally validate a layout (no DB). Returns a human-readable error on failure.
pub fn validate_layout(layout: &Layout) -> Result<(), String> {
    if layout.blocks.len() > MAX_BLOCKS {
        return Err(format!("too many blocks (max {MAX_BLOCKS})"));
    }
    let mut ids = std::collections::HashSet::new();
    for block in &layout.blocks {
        if block.id.trim().is_empty() {
            return Err("a block has an empty id".into());
        }
        if block.id.chars().count() > MAX_BLOCK_ID_LEN {
            return Err("a block id is too long".into());
        }
        if !ids.insert(block.id.clone()) {
            return Err(format!("duplicate block id `{}`", block.id));
        }
        if !BLOCK_KINDS.contains(&block.kind.as_str()) {
            return Err(format!("invalid block kind `{}`", block.kind));
        }
        match block.kind.as_str() {
            "embed" => {
                let it = block
                    .item_type
                    .as_deref()
                    .unwrap_or("")
                    .trim();
                let id = block.item_id.as_deref().unwrap_or("").trim();
                if !EMBED_ITEM_TYPES.contains(&it) {
                    return Err(format!("invalid embed item_type `{it}`"));
                }
                if id.is_empty() {
                    return Err("embed block requires item_id".into());
                }
            }
            "note" => {
                let md = block.markdown.as_deref().unwrap_or("");
                if md.chars().count() > MAX_NOTE {
                    return Err("a note block is too long".into());
                }
            }
            "heading" => {
                let t = block.text.as_deref().unwrap_or("").trim();
                if t.is_empty() {
                    return Err("heading block requires text".into());
                }
                if t.chars().count() > MAX_HEADING {
                    return Err("a heading is too long".into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Plain text for the full-text index: title + notes, headings, and embed titles
/// supplied by the caller after resolving item titles.
pub fn derive_search_text(title: &str, layout: &Layout, embed_titles: &[&str]) -> String {
    let mut parts: Vec<&str> = vec![title];
    for block in &layout.blocks {
        match block.kind.as_str() {
            "note" => {
                if let Some(md) = block.markdown.as_deref() {
                    if !md.is_empty() {
                        parts.push(md);
                    }
                }
            }
            "heading" => {
                if let Some(t) = block.text.as_deref() {
                    parts.push(t);
                }
            }
            _ => {}
        }
    }
    for t in embed_titles {
        if !t.is_empty() {
            parts.push(t);
        }
    }
    parts.join(" ")
}

/// Seed a grid of embed blocks from project member ids (titles resolved later in repo).
pub fn layout_from_member_embeds(
    ideas: &[(String, String)],
    documents: &[(String, String)],
    files: &[(String, String)],
    pages: &[(String, String)],
    mindmaps: &[(String, String)],
    diagrams: &[(String, String)],
) -> Layout {
    let mut blocks: Vec<Block> = Vec::new();
    let mut counter = 0usize;
    let mut col = 0i32;
    let mut row = 0i32;
    let w = 6;
    let h = 4;

    let mut push_embed = |item_type: &str, item_id: &str| {
        counter += 1;
        blocks.push(Block {
            id: format!("b{counter}"),
            kind: "embed".into(),
            x: Some(col * w),
            y: Some(row * h),
            w: Some(w),
            h: Some(h),
            item_type: Some(item_type.to_string()),
            item_id: Some(item_id.to_string()),
            markdown: None,
            text: None,
        });
        col += 1;
        if col >= 2 {
            col = 0;
            row += 1;
        }
    };

    for (id, _) in ideas {
        push_embed("idea", id);
    }
    for (id, _) in documents {
        push_embed("document", id);
    }
    for (id, _) in files {
        push_embed("file", id);
    }
    for (id, _) in pages {
        push_embed("page", id);
    }
    for (id, _) in mindmaps {
        push_embed("mindmap", id);
    }
    for (id, _) in diagrams {
        push_embed("diagram", id);
    }

    Layout {
        layout: "grid".into(),
        blocks,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SuperpageRow {
    pub uuid: String,
    pub owner: String,
    pub title: String,
    pub blocks: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn parse_layout(stored: &str) -> Value {
    serde_json::from_str(stored).unwrap_or_else(|_| serde_json::json!({ "layout": "grid", "blocks": [] }))
}

#[derive(Debug, Serialize)]
pub struct SuperpageDto {
    pub id: String,
    pub title: String,
    pub blocks: Value,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<SuperpageRow> for SuperpageDto {
    fn from(r: SuperpageRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            blocks: parse_layout(&r.blocks),
            folder_id: r.folder_id,
            project_id: r.project_id,
            tags: r.tags,
            review: r.review,
            version: r.version,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SuperpageSummary {
    pub id: String,
    pub title: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub block_count: usize,
    pub updated_at: String,
}

impl SuperpageSummary {
    pub fn from_row(r: SuperpageRow) -> Self {
        let count = serde_json::from_str::<Layout>(&r.blocks)
            .map(|l| l.blocks.len())
            .unwrap_or(0);
        Self {
            id: r.uuid,
            title: r.title,
            folder_id: r.folder_id,
            project_id: r.project_id,
            tags: r.tags,
            review: r.review,
            block_count: count,
            updated_at: r.updated_at,
        }
    }
}