-- 0001_init — foundational schema.
--
-- Phase 1 only establishes the migration plumbing; domain tables (users, files,
-- ideas, documents, …) arrive in later phases. Keep every statement idempotent
-- (`IF NOT EXISTS`) so migrations can be re-applied safely.

-- Records application-level metadata (single row keyed by `app`).
DEFINE TABLE IF NOT EXISTS app_meta SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS schema_version ON app_meta TYPE int;
DEFINE FIELD IF NOT EXISTS updated_at ON app_meta TYPE datetime VALUE time::now();

-- Seed the schema version (no-op on subsequent runs thanks to a fixed id).
UPSERT app_meta:current SET schema_version = 1;
