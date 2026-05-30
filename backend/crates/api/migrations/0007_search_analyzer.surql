-- 0007_search_analyzer — full-text analyzer for cross-type search (Phase 11).
--
-- Split from the SEARCH indexes (0008) on purpose: the migration runner wraps
-- each file in one BEGIN/COMMIT, and a DEFINE INDEX … SEARCH cannot reference an
-- analyzer defined in the *same* uncommitted transaction. Defining the analyzer
-- in its own (earlier) migration commits it before 0008's indexes reference it.
--
-- `class` tokenizer splits on character-class boundaries; `lowercase` + `ascii`
-- make matching case- and accent-insensitive.

DEFINE ANALYZER IF NOT EXISTS kn_text TOKENIZERS class FILTERS lowercase, ascii;
