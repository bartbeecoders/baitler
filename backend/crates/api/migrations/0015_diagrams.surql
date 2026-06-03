-- 0015_diagrams — draw.io (mxGraph) diagrams (Phase 14, Milestone C).
--
-- A `diagram` is a document-like row whose body is mxGraph XML authored in the
-- embedded draw.io editor. It reuses folders/projects/kn_link/knowledge_search/
-- review/activity/export with no bespoke schema, exactly like `mindmap` and
-- `page`. `preview` holds an optional rendered SVG/PNG as a capped `data:` URI
-- (sanitized on render) so lists/cards can show a static thumbnail without the
-- live editor frame. A derived `search_text` column holds the title + text
-- labels extracted from the XML so the full-text index has plain text to match
-- (the live XML is never indexed verbatim).

DEFINE TABLE IF NOT EXISTS diagram SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON diagram TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON diagram TYPE string;
DEFINE FIELD IF NOT EXISTS title ON diagram TYPE string;
DEFINE FIELD IF NOT EXISTS xml ON diagram TYPE string DEFAULT "";            -- mxGraph XML
DEFINE FIELD IF NOT EXISTS preview ON diagram TYPE string DEFAULT "";        -- sanitized SVG/PNG data: URI (optional)
DEFINE FIELD IF NOT EXISTS search_text ON diagram TYPE string DEFAULT "";    -- title + extracted labels (plain text)
DEFINE FIELD IF NOT EXISTS folder_id ON diagram TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS project_id ON diagram TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS tags ON diagram TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS review ON diagram TYPE string DEFAULT "published"; -- draft|published
DEFINE FIELD IF NOT EXISTS version ON diagram TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS published_at ON diagram TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS created_at ON diagram TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON diagram TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS diagram_uuid ON diagram FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS diagram_owner ON diagram FIELDS owner, review;
DEFINE INDEX IF NOT EXISTS diagram_folder ON diagram FIELDS owner, folder_id;
DEFINE INDEX IF NOT EXISTS diagram_project ON diagram FIELDS owner, project_id;
DEFINE INDEX IF NOT EXISTS diagram_tags ON diagram FIELDS tags;

-- Full-text search over the title + extracted labels (the `kn_text` analyzer is
-- committed by migration 0007).
DEFINE INDEX IF NOT EXISTS diagram_ft_title ON diagram FIELDS title SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS diagram_ft_body ON diagram FIELDS search_text SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
