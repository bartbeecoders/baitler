-- 0010_pages — hosted web pages (Phase 12).
--
-- A `page` is a document-like row (sanitized HTML `body`) but, unlike `document`,
-- it is slug-addressed, visibility-gated, and PUBLISHABLE to a served URL
-- (GET /p/{slug}). It reuses the Phase 4 `folder` tree via `folder_id` (no second
-- hierarchy) and the Phase 11 `project_id`/`kn_link` graph. `source_format` is
-- remembered so the editor round-trips Markdown vs HTML. `visibility`:
--   draft     — not served (GET /p/{slug} → 404)
--   unlisted  — served only to link-knowers (slug carries entropy; noindex)
--   public    — served and indexable
-- `body` is sanitized (ammonia) on every write, so even a draft preview is safe;
-- the public serve path additionally runs `convert::harden_for_render`.

DEFINE TABLE IF NOT EXISTS page SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON page TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON page TYPE string;
DEFINE FIELD IF NOT EXISTS title ON page TYPE string;
DEFINE FIELD IF NOT EXISTS body ON page TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS slug ON page TYPE string;
DEFINE FIELD IF NOT EXISTS visibility ON page TYPE string DEFAULT "draft";   -- draft|unlisted|public
DEFINE FIELD IF NOT EXISTS source_format ON page TYPE string DEFAULT "html"; -- html|markdown
DEFINE FIELD IF NOT EXISTS folder_id ON page TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS project_id ON page TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS version ON page TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS published_at ON page TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS created_at ON page TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON page TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS page_uuid ON page FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS page_slug ON page FIELDS owner, slug UNIQUE;
DEFINE INDEX IF NOT EXISTS page_owner ON page FIELDS owner, visibility;
DEFINE INDEX IF NOT EXISTS page_folder ON page FIELDS owner, folder_id;
DEFINE INDEX IF NOT EXISTS page_project ON page FIELDS owner, project_id;

-- Full-text search (the `kn_text` analyzer is committed by migration 0007, and
-- the runner commits each file separately, so referencing it here is safe).
DEFINE INDEX IF NOT EXISTS page_ft_title ON page FIELDS title SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS page_ft_body ON page FIELDS body SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
