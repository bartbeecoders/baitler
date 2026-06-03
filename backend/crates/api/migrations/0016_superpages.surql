-- 0016_superpages — composed knowledge canvas (Phase 15).
--
-- A `superpage` is a document-like row whose structure is a JSON `blocks` body:
-- embed references to other Baitler items, plus local notes/headings. Reuses
-- folders/projects/kn_link/knowledge_search/review/activity with no bespoke schema.

DEFINE TABLE IF NOT EXISTS superpage SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON superpage TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON superpage TYPE string;
DEFINE FIELD IF NOT EXISTS title ON superpage TYPE string;
DEFINE FIELD IF NOT EXISTS blocks ON superpage TYPE string DEFAULT "";          -- JSON { layout, blocks[] }
DEFINE FIELD IF NOT EXISTS search_text ON superpage TYPE string DEFAULT "";    -- title + note/heading/embed titles
DEFINE FIELD IF NOT EXISTS folder_id ON superpage TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS project_id ON superpage TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS tags ON superpage TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS review ON superpage TYPE string DEFAULT "published"; -- draft|published
DEFINE FIELD IF NOT EXISTS version ON superpage TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS published_at ON superpage TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS created_at ON superpage TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON superpage TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS superpage_uuid ON superpage FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS superpage_owner ON superpage FIELDS owner, review;
DEFINE INDEX IF NOT EXISTS superpage_folder ON superpage FIELDS owner, folder_id;
DEFINE INDEX IF NOT EXISTS superpage_project ON superpage FIELDS owner, project_id;
DEFINE INDEX IF NOT EXISTS superpage_tags ON superpage FIELDS tags;

DEFINE INDEX IF NOT EXISTS superpage_ft_title ON superpage FIELDS title SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS superpage_ft_body ON superpage FIELDS search_text SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;