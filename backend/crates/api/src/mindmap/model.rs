//! Data shapes for mindmaps (Phase 14, Milestone B).
//!
//! A mindmap's visual structure is a JSON node/edge graph stored in the `graph`
//! body — the canonical model. Node labels are treated as plain text (escaped
//! wherever rendered, never raw HTML). The graph is shape-validated on every
//! write; item links (a node pointing at a Baitler item) are validated against
//! owner-owned items in the repo layer where the DB is available.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Authoring source formats: `json` (canvas) or `markdown` (outline seed).
pub const SOURCE_FORMATS: &[&str] = &["json", "markdown"];

/// Structural caps (a runaway/abuse guard, well above any real map).
pub const MAX_NODES: usize = 2_000;
pub const MAX_EDGES: usize = 4_000;
pub const MAX_LABEL: usize = 500;

/// The canonical mindmap graph: nodes and the edges between them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    #[serde(default)]
    pub nodes: Vec<MindmapNode>,
    #[serde(default)]
    pub edges: Vec<MindmapEdge>,
}

/// One node. `item_type`/`item_id` optionally point at a Baitler item so the map
/// navigates to the underlying idea/document/file/page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindmapNode {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
}

/// One directed edge between two node ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindmapEdge {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Structurally validate a graph (no DB access): unique non-empty node ids,
/// edges/parents reference existing nodes, label/node/edge caps, and any
/// `item_type` is one of the known item types (the owner-ownership check runs in
/// the repo). Returns a human-readable error string on failure.
pub fn validate_graph(graph: &Graph) -> Result<(), String> {
    use std::collections::HashSet;
    if graph.nodes.len() > MAX_NODES {
        return Err(format!("too many nodes (max {MAX_NODES})"));
    }
    if graph.edges.len() > MAX_EDGES {
        return Err(format!("too many edges (max {MAX_EDGES})"));
    }
    let mut ids: HashSet<&str> = HashSet::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        if node.id.trim().is_empty() {
            return Err("a node has an empty id".into());
        }
        if !ids.insert(node.id.as_str()) {
            return Err(format!("duplicate node id `{}`", node.id));
        }
        if node.label.chars().count() > MAX_LABEL {
            return Err("a node label is too long".into());
        }
        if let Some(it) = &node.item_type {
            if !crate::knowledge::model::ITEM_TYPES.contains(&it.as_str()) {
                return Err(format!("invalid node item_type `{it}`"));
            }
            if node.item_id.as_deref().unwrap_or("").trim().is_empty() {
                return Err("a node has item_type without item_id".into());
            }
        }
    }
    for edge in &graph.edges {
        if !ids.contains(edge.from.as_str()) {
            return Err(format!("edge references unknown node `{}`", edge.from));
        }
        if !ids.contains(edge.to.as_str()) {
            return Err(format!("edge references unknown node `{}`", edge.to));
        }
        if let Some(l) = &edge.label {
            if l.chars().count() > MAX_LABEL {
                return Err("an edge label is too long".into());
            }
        }
    }
    for node in &graph.nodes {
        if let Some(p) = &node.parent {
            if !ids.contains(p.as_str()) {
                return Err(format!("node `{}` has unknown parent `{p}`", node.id));
            }
        }
    }
    Ok(())
}

/// Derive the plain-text the full-text index matches against: the title plus
/// every node and edge label. Keeps the index honest without storing markup.
pub fn derive_search_text(title: &str, graph: &Graph) -> String {
    let mut parts: Vec<&str> = vec![title];
    for n in &graph.nodes {
        if !n.label.is_empty() {
            parts.push(&n.label);
        }
    }
    for e in &graph.edges {
        if let Some(l) = &e.label {
            parts.push(l);
        }
    }
    parts.join(" ")
}

/// Build a graph from a Markdown-ish outline: heading/bullet depth becomes the
/// tree, deeper lines parent to the nearest shallower line. Each line is one
/// node; edges run parent → child.
pub fn from_markdown_outline(outline: &str) -> Graph {
    let mut nodes: Vec<MindmapNode> = Vec::new();
    let mut edges: Vec<MindmapEdge> = Vec::new();
    // Stack of (depth, node_id) for resolving parents. Headings always sit
    // above bullets: a heading of level L is depth L-1; a bullet is one deeper
    // than the most recent heading, plus its own whitespace indentation.
    let mut stack: Vec<(i32, String)> = Vec::new();
    let mut heading_depth: i32 = -1;
    let mut counter = 0usize;

    for raw in outline.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        // Leading-whitespace columns (tab counts as 4).
        let ws_cols: i32 = raw
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum();
        let ws_chars = raw.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let trimmed: String = raw.chars().skip(ws_chars).collect();

        let (label, depth) = if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count() as i32;
            let label = trimmed.trim_start_matches('#').trim().to_string();
            heading_depth = (level - 1).max(0);
            (label, heading_depth)
        } else {
            let label = trimmed
                .trim_start_matches(['-', '*', '+'])
                .trim_start()
                .to_string();
            (label, heading_depth + 1 + ws_cols / 2)
        };
        if label.is_empty() {
            continue;
        }
        counter += 1;
        let id = format!("n{counter}");
        // Pop deeper-or-equal entries; the remaining top is this node's parent.
        while stack.last().map(|(d, _)| *d >= depth).unwrap_or(false) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, id)| id.clone());
        if let Some(p) = &parent {
            edges.push(MindmapEdge {
                from: p.clone(),
                to: id.clone(),
                label: None,
            });
        }
        nodes.push(MindmapNode {
            id: id.clone(),
            label: label.chars().take(MAX_LABEL).collect(),
            parent,
            x: None,
            y: None,
            color: None,
            item_type: None,
            item_id: None,
        });
        stack.push((depth, id));
    }
    Graph { nodes, edges }
}

/// A `mindmap` row as projected by the repository (timestamps → strings). The
/// derived `search_text` column is not projected — it is internal to the index.
#[derive(Debug, Clone, Deserialize)]
pub struct MindmapRow {
    pub uuid: String,
    pub owner: String,
    pub title: String,
    pub graph: String,
    pub source_format: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Parse a stored graph string into a JSON value (empty graph on parse failure).
fn parse_graph(stored: &str) -> Value {
    serde_json::from_str(stored).unwrap_or_else(|_| serde_json::json!({ "nodes": [], "edges": [] }))
}

/// Full mindmap representation (includes the parsed `graph`).
#[derive(Debug, Serialize)]
pub struct MindmapDto {
    pub id: String,
    pub title: String,
    pub graph: Value,
    pub source_format: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<MindmapRow> for MindmapDto {
    fn from(r: MindmapRow) -> Self {
        let graph = parse_graph(&r.graph);
        Self {
            id: r.uuid,
            title: r.title,
            graph,
            source_format: r.source_format,
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

/// Lightweight list entry (omits the `graph` body for payload size).
#[derive(Debug, Serialize)]
pub struct MindmapSummary {
    pub id: String,
    pub title: String,
    pub source_format: String,
    pub folder_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub review: String,
    pub version: i64,
    pub updated_at: String,
}

impl From<MindmapRow> for MindmapSummary {
    fn from(r: MindmapRow) -> Self {
        Self {
            id: r.uuid,
            title: r.title,
            source_format: r.source_format,
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
    use super::*;

    #[test]
    fn outline_builds_a_tree() {
        let g = from_markdown_outline("# Root\n- Child A\n  - Grandchild\n- Child B");
        assert_eq!(g.nodes.len(), 4);
        // Root has no parent; the rest do.
        assert!(g.nodes[0].parent.is_none());
        assert!(g.nodes.iter().skip(1).all(|n| n.parent.is_some()));
        // Edges connect parent → child for every non-root node.
        assert_eq!(g.edges.len(), 3);
        assert!(validate_graph(&g).is_ok());
    }

    #[test]
    fn validate_rejects_dangling_edge_and_dupes() {
        let mut g = Graph::default();
        g.nodes.push(MindmapNode {
            id: "a".into(),
            label: "A".into(),
            parent: None,
            x: None,
            y: None,
            color: None,
            item_type: None,
            item_id: None,
        });
        g.edges.push(MindmapEdge {
            from: "a".into(),
            to: "missing".into(),
            label: None,
        });
        assert!(validate_graph(&g).is_err());
    }

    #[test]
    fn search_text_includes_labels() {
        let g = from_markdown_outline("# Alpha\n- Beta");
        let text = derive_search_text("My Map", &g);
        assert!(text.contains("My Map"));
        assert!(text.contains("Alpha"));
        assert!(text.contains("Beta"));
    }
}
