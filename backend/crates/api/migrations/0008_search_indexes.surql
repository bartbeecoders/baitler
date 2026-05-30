-- 0008_search_indexes — BM25 full-text indexes over existing prose (Phase 11).
--
-- One SEARCH index PER field (the documented multi-field FTS pattern — a single
-- index over multiple fields can't resolve per-field `@N@` matches). HIGHLIGHTS
-- enables search::highlight() snippets. The `kn_text` analyzer is committed by
-- 0007. No schema change to the indexed tables.

DEFINE INDEX IF NOT EXISTS idea_ft_title ON idea FIELDS title SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS idea_ft_body ON idea FIELDS body SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;

DEFINE INDEX IF NOT EXISTS document_ft_title ON document FIELDS title SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS document_ft_body ON document FIELDS body SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;

DEFINE INDEX IF NOT EXISTS project_ft_name ON project FIELDS name SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS project_ft_summary ON project FIELDS summary SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;

DEFINE INDEX IF NOT EXISTS file_ft_name ON file FIELDS name SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
