//! Data shapes for draw.io diagrams (Phase 14, Milestone C).
//!
//! The body is mxGraph XML authored in the embedded draw.io editor. `preview`
//! is an optional rendered SVG/PNG `data:` URI for static thumbnails (rendered
//! in an `<img>`, never executed). A derived `search_text` holds the title plus
//! the text labels extracted from the XML so the full-text index has plain text
//! to match — the raw XML is never indexed verbatim.

use serde::{Deserialize, Serialize};

/// Cap on a stored preview `data:` URI.
pub const MAX_PREVIEW: usize = 4_000_000;

/// Extract the visible text labels from mxGraph XML — the `value="…"` attributes
/// on cells — as one space-joined plain-text string. A tolerant, dependency-free
/// scan (the XML is attacker-authorable, so we never interpret it as markup).
pub fn extract_labels(xml: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let bytes = xml.as_bytes();
    let needle = b"value=\"";
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let start = i + needle.len();
            // Find the closing quote (mxGraph escapes inner quotes as &quot;).
            if let Some(rel) = xml[start..].find('"') {
                let raw = &xml[start..start + rel];
                let text = decode_entities(raw);
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
                i = start + rel + 1;
                continue;
            }
        }
        i += 1;
    }
    out.join(" ")
}

/// Minimal XML entity + tag decode for label text (labels can hold HTML markup
/// in draw.io). Strips tags and unescapes the common entities; the result is
/// plain text fed only to the search index.
fn decode_entities(s: &str) -> String {
    // Strip any HTML tags first (draw.io rich labels embed <div>/<br> etc).
    let mut no_tags = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => no_tags.push(c),
            _ => {}
        }
    }
    no_tags
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
}

/// A `diagram` row as projected by the repository. `search_text` is internal to
/// the index and not projected.
#[derive(Debug, Clone, Deserialize)]
pub struct DiagramRow {
    pub uuid: String,
    pub owner: String,
    pub title: String,
    pub xml: String,
    pub preview: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Full diagram representation (includes the mxGraph `xml` and `preview`).
#[derive(Debug, Serialize)]
pub struct DiagramDto {
    pub id: String,
    pub title: String,
    pub xml: String,
    pub preview: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<DiagramRow> for DiagramDto {
    fn from(r: DiagramRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            xml: r.xml,
            preview: r.preview,
            folder_id: r.folder_id,
            project_id: r.project_id,
            tags: r.tags,
            review: r.review,
            version: r.version,
            published_at: r.published_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Lightweight list entry: omits the `xml` body but keeps the `preview` so cards
/// can show a static thumbnail.
#[derive(Debug, Serialize)]
pub struct DiagramSummary {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub updated_at: String,
}

impl From<DiagramRow> for DiagramSummary {
    fn from(r: DiagramRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            preview: r.preview,
            folder_id: r.folder_id,
            project_id: r.project_id,
            tags: r.tags,
            review: r.review,
            version: r.version,
            updated_at: r.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extract_labels;

    #[test]
    fn pulls_labels_out_of_mxgraph_xml() {
        let xml = r#"<mxGraphModel><root>
          <mxCell id="2" value="Start" vertex="1"/>
          <mxCell id="3" value="&lt;b&gt;Decision&lt;/b&gt;" vertex="1"/>
          <mxCell id="4" value="" edge="1"/>
        </root></mxGraphModel>"#;
        let labels = extract_labels(xml);
        assert!(labels.contains("Start"));
        assert!(labels.contains("Decision"));
        // Empty values are skipped.
        assert!(!labels.contains("  "));
    }
}
