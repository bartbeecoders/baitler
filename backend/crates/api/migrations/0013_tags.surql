-- 0013_tags — cross-type tags on documents + pages (Phase 14).
--
-- `idea` already has `tags` (Phase 5). Add the same flat tag list to `document`
-- and `page` so prose types share one vocabulary, with an index on each so
-- `$tag IN tags` browse stays cheap (SurrealDB indexes each array element).

DEFINE FIELD IF NOT EXISTS tags ON document TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS tags ON page TYPE array<string> DEFAULT [];

DEFINE INDEX IF NOT EXISTS document_tags ON document FIELDS tags;
DEFINE INDEX IF NOT EXISTS page_tags ON page FIELDS tags;
