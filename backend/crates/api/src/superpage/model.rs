//! Data shapes for superpages (Phase 15) — a freeform canvas of typed parts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Part kinds the canvas supports. The last three (`embed`/`note`/`heading`)
/// are legacy v1 kinds kept valid so older boards still load and validate; the
/// authoring UI now creates the typed parts listed first.
pub const BLOCK_KINDS: &[&str] = &[
    "text", "code", "image", "file", "webpage", "mindmap", "diagram",
    // legacy v1 kinds
    "embed", "note", "heading",
];
/// Item types a legacy `embed` block may reference.
pub const EMBED_ITEM_TYPES: &[&str] = &["idea", "document", "file", "page", "mindmap", "diagram"];
/// Where an `image` part's pixels come from.
pub const IMAGE_SOURCES: &[&str] = &["upload", "url", "generated"];
/// What a `webpage` part frames.
pub const WEB_KINDS: &[&str] = &["url", "page"];

pub const MAX_BLOCKS: usize = 200;
pub const MAX_TEXT: usize = 100_000;
pub const MAX_CODE: usize = 100_000;
pub const MAX_LANG: usize = 40;
pub const MAX_URL: usize = 4_096;
pub const MAX_CAPTION: usize = 500;
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
    "canvas".to_string()
}

/// One part on the canvas. Fields are a superset across kinds; `validate_layout`
/// enforces which apply per `kind`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub kind: String,
    // Geometry (pixels on the freeform canvas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    // Reference to a Baitler item (file/page/mindmap/diagram, or legacy embed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    // Inline content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_kind: Option<String>,
}

/// The `(item_type, item_id)` a block references, if any — drives existence
/// checks and full-text title resolution. Returns `None` for purely inline parts.
pub fn referenced(block: &Block) -> Option<(&str, &str)> {
    let id = block
        .item_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match block.kind.as_str() {
        "file" => id.map(|i| ("file", i)),
        "image" if block.src.as_deref() == Some("upload") => id.map(|i| ("file", i)),
        "webpage" if block.web_kind.as_deref() == Some("page") => id.map(|i| ("page", i)),
        "mindmap" => id.map(|i| ("mindmap", i)),
        "diagram" => id.map(|i| ("diagram", i)),
        "embed" => match (block.item_type.as_deref().map(str::trim), id) {
            (Some(it), Some(i)) if !it.is_empty() => Some((it, i)),
            _ => None,
        },
        _ => None,
    }
}

/// Return a copy of the layout with referencing `item_id`/`item_type` trimmed,
/// so the persisted, existence-checked, and resolved ids stay byte-identical.
pub fn normalize_refs(layout: &Layout) -> Layout {
    let mut out = layout.clone();
    for b in &mut out.blocks {
        if let Some(id) = b.item_id.as_mut() {
            let t = id.trim();
            if t.len() != id.len() {
                *id = t.to_string();
            }
        }
        if let Some(it) = b.item_type.as_mut() {
            let t = it.trim();
            if t.len() != it.len() {
                *it = t.to_string();
            }
        }
    }
    out
}

fn require_item_id(block: &Block) -> Result<(), String> {
    let has_id = block
        .item_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if !has_id {
        return Err(format!("{} part requires item_id", block.kind));
    }
    Ok(())
}

/// Accept only safe URL schemes. `data:` is allowed only where `allow_data`
/// (image sources) — never for framed web pages.
fn check_url(url: &str, allow_data: bool) -> Result<(), String> {
    let u = url.trim();
    if u.is_empty() {
        return Err("url must not be empty".into());
    }
    if u.chars().count() > MAX_URL {
        return Err("url is too long".into());
    }
    let lower = u.to_ascii_lowercase();
    let ok = lower.starts_with("http://")
        || lower.starts_with("https://")
        || (allow_data && lower.starts_with("data:"));
    if !ok {
        return Err("url must be http(s)".into());
    }
    Ok(())
}

/// Structurally validate a layout (no DB). Returns a human-readable error on failure.
pub fn validate_layout(layout: &Layout) -> Result<(), String> {
    if layout.blocks.len() > MAX_BLOCKS {
        return Err(format!("too many parts (max {MAX_BLOCKS})"));
    }
    let mut ids = std::collections::HashSet::new();
    for block in &layout.blocks {
        if block.id.trim().is_empty() {
            return Err("a part has an empty id".into());
        }
        if block.id.chars().count() > MAX_BLOCK_ID_LEN {
            return Err("a part id is too long".into());
        }
        if !ids.insert(block.id.clone()) {
            return Err(format!("duplicate part id `{}`", block.id));
        }
        if !BLOCK_KINDS.contains(&block.kind.as_str()) {
            return Err(format!("invalid part kind `{}`", block.kind));
        }
        match block.kind.as_str() {
            "text" => {
                let n = block.markdown.as_deref().unwrap_or("").chars().count();
                if n > MAX_TEXT {
                    return Err("a text part is too long".into());
                }
            }
            "code" => {
                if block.text.as_deref().unwrap_or("").chars().count() > MAX_CODE {
                    return Err("a code part is too long".into());
                }
                if block.lang.as_deref().unwrap_or("").chars().count() > MAX_LANG {
                    return Err("a code language is too long".into());
                }
            }
            "image" => {
                let src = block.src.as_deref().unwrap_or("url");
                if !IMAGE_SOURCES.contains(&src) {
                    return Err(format!("invalid image src `{src}`"));
                }
                if src == "upload" {
                    require_item_id(block)?;
                } else {
                    check_url(block.url.as_deref().unwrap_or(""), true)?;
                }
                if block.text.as_deref().unwrap_or("").chars().count() > MAX_CAPTION {
                    return Err("an image caption is too long".into());
                }
            }
            "file" => require_item_id(block)?,
            "webpage" => {
                let wk = block.web_kind.as_deref().unwrap_or("url");
                if !WEB_KINDS.contains(&wk) {
                    return Err(format!("invalid webpage kind `{wk}`"));
                }
                if wk == "page" {
                    require_item_id(block)?;
                } else {
                    check_url(block.url.as_deref().unwrap_or(""), false)?;
                }
            }
            "mindmap" | "diagram" => require_item_id(block)?,
            // legacy v1 kinds
            "embed" => {
                let it = block.item_type.as_deref().unwrap_or("").trim();
                let id = block.item_id.as_deref().unwrap_or("").trim();
                if !EMBED_ITEM_TYPES.contains(&it) {
                    return Err(format!("invalid embed item_type `{it}`"));
                }
                if id.is_empty() {
                    return Err("embed block requires item_id".into());
                }
            }
            "note" => {
                let n = block.markdown.as_deref().unwrap_or("").chars().count();
                if n > MAX_NOTE {
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

/// Plain text for the full-text index: title + inline text/code/captions, plus
/// referenced item titles supplied by the caller.
pub fn derive_search_text(title: &str, layout: &Layout, ref_titles: &[&str]) -> String {
    let mut parts: Vec<&str> = vec![title];
    for block in &layout.blocks {
        match block.kind.as_str() {
            "text" | "note" => {
                if let Some(md) = block.markdown.as_deref() {
                    if !md.is_empty() {
                        parts.push(md);
                    }
                }
            }
            "heading" | "code" | "image" => {
                if let Some(t) = block.text.as_deref() {
                    if !t.is_empty() {
                        parts.push(t);
                    }
                }
            }
            _ => {}
        }
    }
    for t in ref_titles {
        if !t.is_empty() {
            parts.push(t);
        }
    }
    parts.join(" ")
}

/// Seed a grid of legacy `embed` blocks from project member ids (titles resolved
/// later in the repo). Embeds render read-only; new authoring uses typed parts.
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
    let cw = 400;
    let rh = 300;

    let mut push_embed = |item_type: &str, item_id: &str| {
        counter += 1;
        blocks.push(Block {
            id: format!("b{counter}"),
            kind: "embed".into(),
            x: Some(16 + col * cw),
            y: Some(16 + row * rh),
            w: Some(360),
            h: Some(260),
            item_type: Some(item_type.to_string()),
            item_id: Some(item_id.to_string()),
            markdown: None,
            text: None,
            lang: None,
            url: None,
            src: None,
            web_kind: None,
        });
        col += 1;
        if col >= 3 {
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
        layout: "canvas".into(),
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
    serde_json::from_str(stored)
        .unwrap_or_else(|_| serde_json::json!({ "layout": "canvas", "blocks": [] }))
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
