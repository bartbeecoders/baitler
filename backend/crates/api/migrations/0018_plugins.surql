-- 0018_plugins — agent-authored, human-approved plugins (Phase 16).
--
-- A `plugin` is a content-hashed bundle (validated `manifest` JSON + an
-- optional Extism WASM module) with TWO orthogonal states:
--   review — the human gate (draft|published), reusing the Phase 11 queue;
--   status — the operational state:
--     draft     — proposed (agent or portal); never loaded
--     approved  — human-approved, not yet running
--     enabled   — loaded into the runtime registry (tools live)
--     disabled  — unloaded but kept (cheap re-enable)
--     rejected  — terminal; kept for audit until purged
-- Writes are FORCED to draft/draft regardless of caller args (stricter than
-- `clean_review`, which accepts `published`). The manifest is validated against
-- `plugins::manifest::Manifest` (deny-unknown-fields, default-deny capabilities)
-- before any row is written, mirroring the mindmap graph-in-a-body convention.
-- `wasm_b64` can be large; list/get projections omit it (fetched on load/export).
--
-- `plugin_kv` is the ONLY persistence available to plugin code (host fns
-- `plugin_kv_get`/`plugin_kv_set`): plugins never define tables (SCHEMAFULL +
-- the one-transaction migration runner forbid runtime DDL). Keyed on
-- (owner, slug, k) — slug, not uuid — so state survives a replace/upgrade.

DEFINE TABLE IF NOT EXISTS plugin SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON plugin TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON plugin TYPE string;
DEFINE FIELD IF NOT EXISTS name ON plugin TYPE string;
DEFINE FIELD IF NOT EXISTS slug ON plugin TYPE string;
DEFINE FIELD IF NOT EXISTS description ON plugin TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS version ON plugin TYPE string DEFAULT "0.1.0";
DEFINE FIELD IF NOT EXISTS version_int ON plugin TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS api_version ON plugin TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS manifest ON plugin TYPE string;                 -- validated JSON body
DEFINE FIELD IF NOT EXISTS kinds ON plugin TYPE array<string> DEFAULT [];  -- mcp_tool|api_endpoint|ui_component
DEFINE FIELD IF NOT EXISTS status ON plugin TYPE string DEFAULT "draft";
DEFINE FIELD IF NOT EXISTS review ON plugin TYPE string DEFAULT "draft";
DEFINE FIELD IF NOT EXISTS bundle_sha256 ON plugin TYPE string;
DEFINE FIELD IF NOT EXISTS wasm_b64 ON plugin TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS project_id ON plugin TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS author_agent ON plugin TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS run_id ON plugin TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS created_at ON plugin TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON plugin TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS plugin_uuid ON plugin FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS plugin_slug ON plugin FIELDS owner, slug UNIQUE;
DEFINE INDEX IF NOT EXISTS plugin_owner ON plugin FIELDS owner, status;
DEFINE INDEX IF NOT EXISTS plugin_project ON plugin FIELDS owner, project_id;

-- Full-text search (the `kn_text` analyzer is committed by migration 0007, and
-- the runner commits each file separately, so referencing it here is safe).
DEFINE INDEX IF NOT EXISTS plugin_ft_name ON plugin FIELDS name SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS plugin_ft_description ON plugin FIELDS description SEARCH ANALYZER kn_text BM25 HIGHLIGHTS;

DEFINE TABLE IF NOT EXISTS plugin_kv SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS owner ON plugin_kv TYPE string;
DEFINE FIELD IF NOT EXISTS slug ON plugin_kv TYPE string;
DEFINE FIELD IF NOT EXISTS k ON plugin_kv TYPE string;
DEFINE FIELD IF NOT EXISTS v ON plugin_kv TYPE string;                      -- JSON value, capped at write
DEFINE FIELD IF NOT EXISTS updated_at ON plugin_kv TYPE datetime VALUE time::now();

DEFINE INDEX IF NOT EXISTS plugin_kv_key ON plugin_kv FIELDS owner, slug, k UNIQUE;
