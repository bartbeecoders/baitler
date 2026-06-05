//! Resolve superpage parts into agent-friendly, renderable previews.

use serde::Serialize;
use serde_json::{json, Value};

use crate::db::Db;
use crate::diagrams::repo as diagrams_repo;
use crate::documents::repo as doc_repo;
use crate::error::AppResult;
use crate::files::repo as files_repo;
use crate::ideas::repo as ideas_repo;
use crate::knowledge::repo as kn;
use crate::mindmap::repo as mindmap_repo;
use crate::pages::model::public_path;
use crate::pages::repo as pages_repo;

use super::model::{Block, Layout};

const MAX_BODY_PREVIEW: usize = 8_000;
const MAX_MARKDOWN_PREVIEW: usize = 4_000;

#[derive(Debug, Serialize)]
pub struct ResolvedBlock {
    pub id: String,
    pub kind: String,
    #[serde(flatten)]
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct SuperpageContext {
    pub id: String,
    pub title: String,
    pub blocks: Vec<ResolvedBlock>,
}

pub async fn resolve_context(
    db: &Db,
    owner: &str,
    id: &str,
    title: &str,
    layout: &Layout,
) -> AppResult<SuperpageContext> {
    let mut resolved = Vec::with_capacity(layout.blocks.len());
    for block in &layout.blocks {
        resolved.push(ResolvedBlock {
            id: block.id.clone(),
            kind: block.kind.clone(),
            payload: resolve_block(db, owner, block).await?,
        });
    }
    Ok(SuperpageContext {
        id: id.to_string(),
        title: title.to_string(),
        blocks: resolved,
    })
}

async fn resolve_block(db: &Db, owner: &str, block: &Block) -> AppResult<Value> {
    match block.kind.as_str() {
        "text" | "note" => Ok(json!({ "markdown": block.markdown.as_deref().unwrap_or("") })),
        "heading" => Ok(json!({ "text": block.text.as_deref().unwrap_or("") })),
        "code" => Ok(json!({
            "text": block.text.as_deref().unwrap_or(""),
            "lang": block.lang.as_deref().unwrap_or(""),
        })),
        "image" => resolve_image(db, owner, block).await,
        "file" => match block.item_id.as_deref() {
            Some(id) => resolve_file(db, owner, id).await,
            None => Ok(json!({ "missing": true })),
        },
        "webpage" => resolve_webpage(db, owner, block).await,
        "mindmap" => match block.item_id.as_deref() {
            Some(id) => resolve_mindmap(db, owner, id).await,
            None => Ok(json!({ "missing": true })),
        },
        "diagram" => match block.item_id.as_deref() {
            Some(id) => resolve_diagram(db, owner, id).await,
            None => Ok(json!({ "missing": true })),
        },
        "embed" => {
            let (Some(it), Some(item_id)) = (&block.item_type, &block.item_id) else {
                return Ok(json!({ "error": "incomplete embed" }));
            };
            resolve_embed(db, owner, it, item_id).await
        }
        _ => Ok(json!({})),
    }
}

async fn resolve_image(db: &Db, owner: &str, block: &Block) -> AppResult<Value> {
    let src = block.src.as_deref().unwrap_or("url");
    if src == "upload" {
        let Some(id) = block.item_id.as_deref() else {
            return Ok(json!({ "src": "upload", "missing": true }));
        };
        let file = files_repo::get_file(db, owner, id).await?;
        Ok(json!({
            "src": "upload",
            "item_id": id,
            "missing": file.is_none(),
            "title": file.as_ref().map(|f| f.name.clone()),
            "mime": file.as_ref().map(|f| f.mime.clone()),
            "size": file.as_ref().map(|f| f.size),
            "caption": block.text,
        }))
    } else {
        Ok(json!({
            "src": src,
            "url": block.url,
            "caption": block.text,
        }))
    }
}

async fn resolve_file(db: &Db, owner: &str, item_id: &str) -> AppResult<Value> {
    let file = files_repo::get_file(db, owner, item_id).await?;
    Ok(json!({
        "item_id": item_id,
        "missing": file.is_none(),
        "title": file.as_ref().map(|f| f.name.clone()),
        "name": file.as_ref().map(|f| f.name.clone()),
        "mime": file.as_ref().map(|f| f.mime.clone()),
        "size": file.as_ref().map(|f| f.size),
    }))
}

async fn resolve_webpage(db: &Db, owner: &str, block: &Block) -> AppResult<Value> {
    let wk = block.web_kind.as_deref().unwrap_or("url");
    if wk == "page" {
        let Some(id) = block.item_id.as_deref() else {
            return Ok(json!({ "web_kind": "page", "missing": true }));
        };
        let page = pages_repo::get_page(db, owner, id).await?;
        let (title, body, public_url, visibility) = page
            .map(|p| {
                (
                    Some(p.title),
                    truncate(&p.body, MAX_BODY_PREVIEW),
                    public_path(&p.slug, &p.visibility),
                    p.visibility,
                )
            })
            .unwrap_or((None, String::new(), String::new(), String::new()));
        Ok(json!({
            "web_kind": "page",
            "item_id": id,
            "missing": title.is_none(),
            "title": title,
            "format": "html",
            "body": body,
            "public_url": public_url,
            "visibility": visibility,
        }))
    } else {
        Ok(json!({ "web_kind": "url", "url": block.url }))
    }
}

async fn resolve_mindmap(db: &Db, owner: &str, item_id: &str) -> AppResult<Value> {
    let mm = mindmap_repo::get_mindmap(db, owner, item_id).await?;
    let title = mm.as_ref().map(|m| m.title.clone());
    let graph = mm
        .map(|m| serde_json::from_str::<Value>(&m.graph).unwrap_or(json!({})))
        .unwrap_or(json!({}));
    Ok(json!({
        "item_id": item_id,
        "missing": title.is_none(),
        "title": title,
        "graph": graph,
    }))
}

async fn resolve_diagram(db: &Db, owner: &str, item_id: &str) -> AppResult<Value> {
    let d = diagrams_repo::get_diagram(db, owner, item_id).await?;
    let title = d.as_ref().map(|x| x.title.clone());
    let preview = d
        .as_ref()
        .map(|x| x.preview.clone())
        .filter(|p| !p.is_empty());
    let has_xml = d.as_ref().is_some_and(|x| !x.xml.is_empty());
    Ok(json!({
        "item_id": item_id,
        "missing": title.is_none(),
        "title": title,
        "has_xml": has_xml,
        "preview": preview,
    }))
}

/// Legacy v1 `embed` block resolution (idea/document plus the typed items).
async fn resolve_embed(db: &Db, owner: &str, item_type: &str, item_id: &str) -> AppResult<Value> {
    let title = kn::resolve_title(db, owner, item_type, item_id).await?;
    let Some(title) = title else {
        return Ok(json!({
            "item_type": item_type,
            "item_id": item_id,
            "missing": true,
        }));
    };

    match item_type {
        "idea" => {
            let idea = ideas_repo::get_idea(db, owner, item_id).await?;
            let body = idea
                .map(|i| truncate(&i.body, MAX_MARKDOWN_PREVIEW))
                .unwrap_or_default();
            Ok(json!({
                "item_type": item_type,
                "item_id": item_id,
                "title": title,
                "format": "markdown",
                "body": body,
            }))
        }
        "document" => {
            let doc = doc_repo::get_document(db, owner, item_id).await?;
            let body = doc
                .map(|d| truncate(&d.body, MAX_BODY_PREVIEW))
                .unwrap_or_default();
            Ok(json!({
                "item_type": item_type,
                "item_id": item_id,
                "title": title,
                "format": "html",
                "body": body,
            }))
        }
        "page" => {
            let page = pages_repo::get_page(db, owner, item_id).await?;
            let (body, public_url, visibility) = page
                .map(|p| {
                    (
                        truncate(&p.body, MAX_BODY_PREVIEW),
                        public_path(&p.slug, &p.visibility),
                        p.visibility,
                    )
                })
                .unwrap_or_else(|| (String::new(), String::new(), String::new()));
            Ok(json!({
                "item_type": item_type,
                "item_id": item_id,
                "title": title,
                "format": "html",
                "body": body,
                "public_url": public_url,
                "visibility": visibility,
            }))
        }
        "file" => {
            let file = files_repo::get_file(db, owner, item_id).await?;
            Ok(json!({
                "item_type": item_type,
                "item_id": item_id,
                "title": title,
                "name": file.as_ref().map(|f| f.name.clone()),
                "mime": file.as_ref().map(|f| f.mime.clone()),
                "size": file.as_ref().map(|f| f.size),
            }))
        }
        "mindmap" => {
            let mm = mindmap_repo::get_mindmap(db, owner, item_id).await?;
            let graph = mm
                .map(|m| serde_json::from_str::<Value>(&m.graph).unwrap_or(json!({})))
                .unwrap_or(json!({}));
            Ok(json!({
                "item_type": item_type,
                "item_id": item_id,
                "title": title,
                "graph": graph,
            }))
        }
        "diagram" => {
            let d = diagrams_repo::get_diagram(db, owner, item_id).await?;
            let preview = d
                .as_ref()
                .map(|x| x.preview.clone())
                .filter(|p| !p.is_empty());
            let has_xml = d.as_ref().is_some_and(|x| !x.xml.is_empty());
            Ok(json!({
                "item_type": item_type,
                "item_id": item_id,
                "title": title,
                "has_xml": has_xml,
                "preview": preview,
            }))
        }
        _ => Ok(json!({
            "item_type": item_type,
            "item_id": item_id,
            "title": title,
        })),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("hi", 10), "hi");
        assert!(truncate(&"x".repeat(20), 5).ends_with('…'));
    }
}
