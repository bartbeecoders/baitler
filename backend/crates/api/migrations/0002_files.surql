-- 0002_files — file storage & folders.
--
-- Public ids are the `uuid` fields (server-generated), not SurrealDB record ids,
-- so the API never exposes or parses internal record ids. `owner` scopes every
-- row to its user (a fixed dev owner until auth lands). Hierarchy is by
-- `parent_id` / `folder_id` referencing a folder's `uuid` (NONE = root).
-- Timestamps are maintained by the schema: created_at set once, updated_at on
-- every write.

DEFINE TABLE IF NOT EXISTS folder SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON folder TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON folder TYPE string;
DEFINE FIELD IF NOT EXISTS name ON folder TYPE string;
DEFINE FIELD IF NOT EXISTS parent_id ON folder TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON folder TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON folder TYPE datetime VALUE time::now();
DEFINE INDEX IF NOT EXISTS folder_uuid ON folder FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS folder_owner ON folder FIELDS owner, parent_id;

DEFINE TABLE IF NOT EXISTS file SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON file TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON file TYPE string;
DEFINE FIELD IF NOT EXISTS name ON file TYPE string;
DEFINE FIELD IF NOT EXISTS mime ON file TYPE string;
DEFINE FIELD IF NOT EXISTS size ON file TYPE int;
DEFINE FIELD IF NOT EXISTS folder_id ON file TYPE option<string>;
DEFINE FIELD IF NOT EXISTS storage_key ON file TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON file TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON file TYPE datetime VALUE time::now();
DEFINE INDEX IF NOT EXISTS file_uuid ON file FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS file_owner ON file FIELDS owner, folder_id;
