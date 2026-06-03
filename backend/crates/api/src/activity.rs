//! Append-only activity / provenance log (Phase 11).
//!
//! Every mutating MCP tool call records one row here (who = `agent`, what =
//! `action` + target), giving each piece of agent-created content a provenance
//! trail and powering the portal's activity timeline. Owner-scoped like
//! everything else. Reads never write; the log is immutable from the app side.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::Db;
use crate::error::AppResult;

const ACTIVITY_SELECT: &str = "SELECT uuid, owner, agent, action, target_type, target_id, \
    target_title, project_id, summary, type::string(created_at) AS created_at FROM activity";

/// An `activity` row as projected by the repository.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActivityRow {
    pub uuid: String,
    pub owner: String,
    pub agent: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub target_title: String,
    pub project_id: Option<String>,
    pub summary: String,
    pub created_at: String,
}

/// Public activity representation.
#[derive(Debug, Serialize)]
pub struct ActivityDto {
    pub id: String,
    pub agent: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub target_title: String,
    pub project_id: Option<String>,
    pub summary: String,
    pub created_at: String,
}

impl From<ActivityRow> for ActivityDto {
    fn from(r: ActivityRow) -> Self {
        Self {
            id: r.uuid,
            agent: r.agent,
            action: r.action,
            target_type: r.target_type,
            target_id: r.target_id,
            target_title: r.target_title,
            project_id: r.project_id,
            summary: r.summary,
            created_at: r.created_at,
        }
    }
}

/// A new activity record (borrowed fields; built at the call site).
pub struct NewActivity<'a> {
    pub action: &'a str,
    pub target_type: &'a str,
    pub target_id: &'a str,
    pub target_title: &'a str,
    pub project_id: Option<&'a str>,
    pub summary: &'a str,
}

/// An owned activity record derived from a tool result.
pub struct ActivityEntry {
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub target_title: String,
    pub project_id: Option<String>,
    pub summary: String,
}

impl ActivityEntry {
    pub fn as_new(&self) -> NewActivity<'_> {
        NewActivity {
            action: &self.action,
            target_type: &self.target_type,
            target_id: &self.target_id,
            target_title: &self.target_title,
            project_id: self.project_id.as_deref(),
            summary: &self.summary,
        }
    }
}

/// Map a successful MCP tool call to an activity record. Returns `None` for
/// read-only tools (which record nothing). The target id/title come from the
/// tool's own result value — never from raw arguments.
pub fn entry_for(tool: &str, result: &Value) -> Option<ActivityEntry> {
    let (action, target_type): (&str, &str) = match tool {
        "ideas_create" => ("idea.create", "idea"),
        "ideas_update" => ("idea.update", "idea"),
        "ideas_delete" => ("idea.delete", "idea"),
        "ideas_link" => ("idea.link", "idea"),
        "ideas_unlink" => ("idea.unlink", "idea"),
        "documents_create" => ("document.create", "document"),
        "documents_update" => ("document.update", "document"),
        "documents_delete" => ("document.delete", "document"),
        "documents_publish" => ("document.publish", "document"),
        "pages_create" => ("page.create", "page"),
        "pages_update" => ("page.update", "page"),
        "pages_delete" => ("page.delete", "page"),
        "pages_publish" => ("page.publish", "page"),
        "pages_unpublish" => ("page.unpublish", "page"),
        "mindmaps_create" | "mindmaps_from_project" => ("mindmap.create", "mindmap"),
        "mindmaps_update" => ("mindmap.update", "mindmap"),
        "mindmaps_delete" => ("mindmap.delete", "mindmap"),
        "diagrams_create" => ("diagram.create", "diagram"),
        "diagrams_update" => ("diagram.update", "diagram"),
        "diagrams_delete" => ("diagram.delete", "diagram"),
        "collection_export" => ("project.export", "project"),
        "files_write" => ("file.create", "file"),
        "files_import" => ("file.import", "file"),
        "files_delete" => ("file.delete", "file"),
        "folders_create" => ("folder.create", "folder"),
        "projects_create" => ("project.create", "project"),
        "projects_update" => ("project.update", "project"),
        "projects_delete" => ("project.delete", "project"),
        "projects_add_item" => ("project.add_item", "project"),
        "projects_remove_item" => ("project.remove_item", "project"),
        "knowledge_link" => ("knowledge.link", "link"),
        "knowledge_unlink" => ("knowledge.unlink", "link"),
        // Everything else (list/get/read/search/export/health/ai_*) is read-only.
        _ => return None,
    };
    let field = |k: &str| result.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let target_id = field("id").unwrap_or_default();
    let target_title = field("title").or_else(|| field("name")).unwrap_or_default();
    let project_id = field("project_id");
    let summary = if target_title.is_empty() {
        action.to_string()
    } else {
        format!("{action} · {target_title}")
    };
    Some(ActivityEntry {
        action: action.to_string(),
        target_type: target_type.to_string(),
        target_id,
        target_title,
        project_id,
        summary,
    })
}

/// Append one activity row. `agent` is `None` for human/web actions.
pub async fn record(
    db: &Db,
    owner: &str,
    agent: Option<&str>,
    a: &NewActivity<'_>,
) -> AppResult<()> {
    db.query(
        "CREATE activity CONTENT { uuid: $uuid, owner: $owner, agent: $agent, action: $action, \
         target_type: $tt, target_id: $tid, target_title: $title, project_id: $pid, \
         summary: $summary }",
    )
    .bind(("uuid", Uuid::new_v4().to_string()))
    .bind(("owner", owner.to_string()))
    .bind(("agent", agent.map(str::to_string)))
    .bind(("action", a.action.to_string()))
    .bind(("tt", a.target_type.to_string()))
    .bind(("tid", a.target_id.to_string()))
    .bind(("title", a.target_title.to_string()))
    .bind(("pid", a.project_id.map(str::to_string)))
    .bind(("summary", a.summary.to_string()))
    .await?
    .check()?;
    Ok(())
}

/// List activity newest-first, optionally filtered by project, agent, or a
/// minimum ISO-8601 timestamp (`since`).
pub async fn list(
    db: &Db,
    owner: &str,
    project_id: Option<&str>,
    agent: Option<&str>,
    since: Option<&str>,
    limit: usize,
) -> AppResult<Vec<ActivityRow>> {
    let mut clauses = vec!["owner = $owner".to_string()];
    if project_id.is_some() {
        clauses.push("project_id = $pid".to_string());
    }
    if agent.is_some() {
        clauses.push("agent = $agent".to_string());
    }
    if since.is_some() {
        clauses.push("created_at >= type::datetime($since)".to_string());
    }
    let sql = format!(
        "{ACTIVITY_SELECT} WHERE {} ORDER BY created_at DESC LIMIT $limit",
        clauses.join(" AND ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("limit", limit as i64));
    if let Some(p) = project_id {
        query = query.bind(("pid", p.to_string()));
    }
    if let Some(a) = agent {
        query = query.bind(("agent", a.to_string()));
    }
    if let Some(s) = since {
        query = query.bind(("since", s.to_string()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}
