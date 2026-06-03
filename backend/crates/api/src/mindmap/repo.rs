//! SurrealDB persistence for mindmaps. Owner-scoped. The `graph` is stored as a
//! canonical JSON string; a derived `search_text` (title + node/edge labels)
//! feeds the full-text index. Item links on nodes are validated against
//! owner-owned items here (where the DB is available).

use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::knowledge::repo as kn;

use super::model::{derive_search_text, validate_graph, Graph, MindmapNode, MindmapRow};

const MINDMAP_SELECT: &str = "SELECT uuid, owner, title, graph, source_format, folder_id, \
    project_id, tags, review, version, \
    IF published_at = NONE THEN NONE ELSE type::string(published_at) END AS published_at, \
    type::string(created_at) AS created_at, type::string(updated_at) AS updated_at FROM mindmap";

fn vanished() -> AppError {
    AppError::Internal("mindmap not found immediately after write".into())
}

/// Structurally validate a graph, then verify every node item-link points at an
/// owner-owned item. Returns the canonical JSON string + derived search text.
async fn prepare_graph(db: &Db, owner: &str, graph: &Graph) -> AppResult<(String, String)> {
    validate_graph(graph).map_err(AppError::BadRequest)?;
    for node in &graph.nodes {
        if let (Some(it), Some(id)) = (&node.item_type, &node.item_id) {
            if !kn::item_exists(db, owner, it, id).await? {
                return Err(AppError::BadRequest(format!(
                    "node links to a missing {it} `{id}`"
                )));
            }
        }
    }
    let json = serde_json::to_string(graph)
        .map_err(|e| AppError::Internal(format!("graph serialize failed: {e}").into()))?;
    Ok((json, derive_search_text("", graph)))
}

#[allow(clippy::too_many_arguments)]
pub async fn create_mindmap(
    db: &Db,
    owner: &str,
    title: &str,
    graph: &Graph,
    source_format: &str,
    folder_id: Option<&str>,
    project_id: Option<&str>,
    tags: &[String],
    review: &str,
) -> AppResult<MindmapRow> {
    let uuid = Uuid::new_v4().to_string();
    let (graph_json, labels) = prepare_graph(db, owner, graph).await?;
    let search_text = format!("{title} {labels}");
    let sql = format!(
        "CREATE mindmap CONTENT {{ uuid: $uuid, owner: $owner, title: $title, graph: $graph, \
         search_text: $search_text, source_format: $source_format, folder_id: $folder_id, \
         project_id: $project_id, tags: $tags, review: $review }}; \
         {MINDMAP_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("title", title.to_string()))
        .bind(("graph", graph_json))
        .bind(("search_text", search_text))
        .bind(("source_format", source_format.to_string()))
        .bind(("folder_id", folder_id.map(str::to_string)))
        .bind(("project_id", project_id.map(str::to_string)))
        .bind(("tags", tags.to_vec()))
        .bind(("review", review.to_string()))
        .await?
        .check()?;
    let rows: Vec<MindmapRow> = res.take(1)?;
    rows.into_iter().next().ok_or_else(vanished)
}

pub async fn get_mindmap(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<MindmapRow>> {
    let sql = format!("{MINDMAP_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<MindmapRow>>(0)?.into_iter().next())
}

#[allow(clippy::too_many_arguments)]
pub async fn list_mindmaps(
    db: &Db,
    owner: &str,
    folder: Option<&str>,
    project: Option<&str>,
    tag: Option<&str>,
    review: Option<&str>,
    q: Option<&str>,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<MindmapRow>> {
    let mut clauses = vec!["owner = $owner".to_string()];
    if folder.is_some() {
        clauses.push("folder_id = $folder".to_string());
    }
    if project.is_some() {
        clauses.push("project_id = $project".to_string());
    }
    if tag.is_some() {
        clauses.push("$tag IN tags".to_string());
    }
    if review.is_some() {
        clauses.push("review = $review".to_string());
    }
    if q.is_some() {
        clauses.push(
            "(string::lowercase(title) CONTAINS string::lowercase($q) \
             OR string::lowercase(search_text) CONTAINS string::lowercase($q))"
                .to_string(),
        );
    }
    let sql = format!(
        "{MINDMAP_SELECT} WHERE {} ORDER BY updated_at DESC LIMIT $limit START $offset",
        clauses.join(" AND ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64));
    if let Some(f) = folder {
        query = query.bind(("folder", f.to_string()));
    }
    if let Some(p) = project {
        query = query.bind(("project", p.to_string()));
    }
    if let Some(t) = tag {
        query = query.bind(("tag", t.to_string()));
    }
    if let Some(r) = review {
        query = query.bind(("review", r.to_string()));
    }
    if let Some(s) = q {
        query = query.bind(("q", s.to_string()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

/// Distinct tags used across this owner's mindmaps.
pub async fn distinct_tags(db: &Db, owner: &str) -> AppResult<Vec<Vec<String>>> {
    let mut res = db
        .query("SELECT VALUE tags FROM mindmap WHERE owner = $owner")
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}

/// A partial update. `folder_id`/`project_id` use a double-option:
/// `None` = leave, `Some(None)` = clear, `Some(Some(id))` = set.
#[derive(Default)]
pub struct MindmapPatch<'a> {
    pub title: Option<&'a str>,
    pub graph: Option<&'a Graph>,
    pub source_format: Option<&'a str>,
    pub review: Option<&'a str>,
    pub folder_id: Option<Option<&'a str>>,
    pub project_id: Option<Option<&'a str>>,
    pub tags: Option<&'a [String]>,
}

pub async fn update_mindmap(
    db: &Db,
    owner: &str,
    uuid: &str,
    patch: MindmapPatch<'_>,
) -> AppResult<Option<MindmapRow>> {
    let Some(current) = get_mindmap(db, owner, uuid).await? else {
        return Ok(None);
    };

    // Re-prepare the graph (validate + item links) if a new one is supplied.
    let prepared = match patch.graph {
        Some(g) => Some(prepare_graph(db, owner, g).await?),
        None => None,
    };

    let mut sets: Vec<&str> = Vec::new();
    if patch.title.is_some() {
        sets.push("title = $title");
    }
    if prepared.is_some() {
        sets.push("graph = $graph");
    }
    // search_text tracks the title + the (possibly new) graph labels.
    if patch.title.is_some() || prepared.is_some() {
        sets.push("search_text = $search_text");
    }
    if patch.source_format.is_some() {
        sets.push("source_format = $source_format");
    }
    if patch.review.is_some() {
        sets.push("review = $review");
    }
    match patch.folder_id {
        Some(Some(_)) => sets.push("folder_id = $folder"),
        Some(None) => sets.push("folder_id = NONE"),
        None => {}
    }
    match patch.project_id {
        Some(Some(_)) => sets.push("project_id = $project"),
        Some(None) => sets.push("project_id = NONE"),
        None => {}
    }
    if patch.tags.is_some() {
        sets.push("tags = $tags");
    }
    // Version bumps on a content edit only (title or graph).
    if patch.title.is_some() || prepared.is_some() {
        sets.push("version = version + 1");
    }
    if sets.is_empty() {
        return Ok(Some(current));
    }

    // Derive the search text from whichever of title/graph is changing, falling
    // back to the current values. The graph labels come from the prepared pass.
    let new_title = patch.title.unwrap_or(&current.title).to_string();
    let labels = match &prepared {
        Some((_, labels)) => labels.clone(),
        None => derive_search_text("", &current_graph(&current)),
    };
    let search_text = format!("{new_title} {labels}");

    let sql = format!(
        "UPDATE mindmap SET {} WHERE owner = $owner AND uuid = $uuid; \
         {MINDMAP_SELECT} WHERE owner = $owner AND uuid = $uuid",
        sets.join(", ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()));
    if let Some(t) = patch.title {
        query = query.bind(("title", t.to_string()));
    }
    if let Some((json, _)) = &prepared {
        query = query.bind(("graph", json.clone()));
    }
    if patch.title.is_some() || prepared.is_some() {
        query = query.bind(("search_text", search_text));
    }
    if let Some(f) = patch.source_format {
        query = query.bind(("source_format", f.to_string()));
    }
    if let Some(r) = patch.review {
        query = query.bind(("review", r.to_string()));
    }
    if let Some(Some(f)) = patch.folder_id {
        query = query.bind(("folder", f.to_string()));
    }
    if let Some(Some(p)) = patch.project_id {
        query = query.bind(("project", p.to_string()));
    }
    if let Some(t) = patch.tags {
        query = query.bind(("tags", t.to_vec()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take::<Vec<MindmapRow>>(1)?.into_iter().next())
}

/// Parse a row's stored graph back into a `Graph` (empty on failure).
fn current_graph(row: &MindmapRow) -> Graph {
    serde_json::from_str(&row.graph).unwrap_or_default()
}

/// Delete a mindmap, scrubbing its cross-type knowledge links first.
pub async fn delete_mindmap(db: &Db, owner: &str, uuid: &str) -> AppResult<bool> {
    if get_mindmap(db, owner, uuid).await?.is_none() {
        return Ok(false);
    }
    kn::scrub_item_links(db, owner, "mindmap", uuid).await?;
    db.query("DELETE mindmap WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(true)
}

/// Seed a graph from a project: a central project node, one node per member
/// idea laid out radially, membership edges (project → idea), and idea↔idea
/// edges mirroring their `kn_link`s. Returns the graph (the caller persists it).
pub async fn seed_from_project(db: &Db, owner: &str, project_id: &str) -> AppResult<Graph> {
    let project = kn::get_project(db, owner, project_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let members = kn::project_members(db, owner, project_id).await?;

    let mut nodes: Vec<MindmapNode> = Vec::new();
    let mut edges: Vec<super::model::MindmapEdge> = Vec::new();

    nodes.push(MindmapNode {
        id: "root".into(),
        label: project.name.clone(),
        parent: None,
        x: Some(0.0),
        y: Some(0.0),
        color: None,
        item_type: Some("project".into()),
        item_id: Some(project_id.to_string()),
    });

    let ideas = &members.ideas;
    let n = ideas.len().max(1) as f64;
    let radius = 280.0;
    for (i, idea) in ideas.iter().enumerate() {
        let angle = std::f64::consts::TAU * (i as f64) / n;
        nodes.push(MindmapNode {
            id: idea.id.clone(),
            label: idea.title.clone(),
            parent: Some("root".into()),
            x: Some(radius * angle.cos()),
            y: Some(radius * angle.sin()),
            color: None,
            item_type: Some("idea".into()),
            item_id: Some(idea.id.clone()),
        });
        edges.push(super::model::MindmapEdge {
            from: "root".into(),
            to: idea.id.clone(),
            label: None,
        });
    }

    // idea↔idea kn_links become edges (deduped; only within the member set).
    use std::collections::HashSet;
    let member_ids: HashSet<&str> = ideas.iter().map(|i| i.id.as_str()).collect();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for idea in ideas {
        for link in kn::backlinks(db, owner, "idea", &idea.id).await? {
            if link.item_type != "idea" || !member_ids.contains(link.id.as_str()) {
                continue;
            }
            // Canonical undirected key so the symmetric pair isn't drawn twice.
            let (a, b) = if idea.id <= link.id {
                (idea.id.clone(), link.id.clone())
            } else {
                (link.id.clone(), idea.id.clone())
            };
            if seen.insert((a.clone(), b.clone())) {
                edges.push(super::model::MindmapEdge {
                    from: a,
                    to: b,
                    label: if link.relation.is_empty() {
                        None
                    } else {
                        Some(link.relation)
                    },
                });
            }
        }
    }

    Ok(Graph { nodes, edges })
}
