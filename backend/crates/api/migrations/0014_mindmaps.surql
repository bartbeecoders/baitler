-- 0014_mindmaps — visual idea maps (Phase 14, Milestone B).
--
-- A `mindmap` is a document-like row (owner/title/folder_id/project_id/tags/
-- review/version/timestamps) whose visual structure lives in a JSON `graph`
-- body — `{ nodes:[{ id, label, parent?, x?, y?, color?, item_type?, item_id? }],
-- edges:[{ from, to, label? }] }` — exactly as a document's structure lives in
-- its HTML body, so it reuses folders/projects/kn_link/knowledge_search/review/
-- activity/export with no bespoke graph schema. `source_format` is `json`
-- (authored in the canvas) or `markdown` (seeded from an outline). A derived
-- `search_text` column holds the title + node labels as plain text so the
-- full-text index has something to match (node labels are escaped on render,
-- never raw HTML). `published_at` is carried for symmetry with documents/pages
-- though mindmaps are not URL-served.

DEFINE TABLE IF NOT EXISTS mindmap SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON mindmap TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON mindmap TYPE string;
DEFINE FIELD IF NOT EXISTS title ON mindmap TYPE string;
DEFINE FIELD IF NOT EXISTS graph ON mindmap TYPE string DEFAULT "";          -- JSON { nodes, edges }
DEFINE FIELD IF NOT EXISTS search_text ON mindmap TYPE string DEFAULT "";    -- title + node labels (plain text)
DEFINE FIELD IF NOT EXISTS source_format ON mindmap TYPE string DEFAULT "json"; -- json|markdown
DEFINE FIELD IF NOT EXISTS folder_id ON mindmap TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS project_id ON mindmap TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS tags ON mindmap TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS review ON mindmap TYPE string DEFAULT "published"; -- draft|published
DEFINE FIELD IF NOT EXISTS version ON mindmap TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS published_at ON mindmap TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS created_at ON mindmap TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON mindmap TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS mindmap_uuid ON mindmap FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS mindmap_owner ON mindmap FIELDS owner, review;
DEFINE INDEX IF NOT EXISTS mindmap_folder ON mindmap FIELDS owner, folder_id;
DEFINE INDEX IF NOT EXISTS mindmap_project ON mindmap FIELDS owner, project_id;
DEFINE INDEX IF NOT EXISTS mindmap_tags ON mindmap FIELDS tags;

-- Full-text search over the title + derived node-label text (the `kn_text`
-- analyzer is committed by migration 0007; the runner commits each file
-- separately, so referencing it here is safe).
DEFINE INDEX IF NOT EXISTS mindmap_ft_title ON mindmap FIELDS title SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS mindmap_ft_body ON mindmap FIELDS search_text SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
