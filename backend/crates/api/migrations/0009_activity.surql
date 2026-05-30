-- 0008_activity — append-only activity / provenance log (Phase 11).
--
-- One row per mutating MCP tool call (and, later, portal action). `agent` is the
-- calling agent label (NONE = a human/web action). Rows are never updated or
-- deleted by the app — this is the audit trail that powers provenance badges and
-- the portal activity timeline. We store a derived `summary` + target id only,
-- never the verbatim tool-argument bodies.

DEFINE TABLE IF NOT EXISTS activity SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON activity TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON activity TYPE string;
DEFINE FIELD IF NOT EXISTS agent ON activity TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS action ON activity TYPE string;          -- e.g. idea.create
DEFINE FIELD IF NOT EXISTS target_type ON activity TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS target_id ON activity TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS target_title ON activity TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS project_id ON activity TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS summary ON activity TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS created_at ON activity TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS activity_uuid ON activity FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS activity_owner ON activity FIELDS owner, created_at;
