-- 0005_documents — HTML document editor storage.
--
-- `body` is sanitized HTML (see src/convert / ammonia). `version` increments on
-- each save (simple optimistic counter; full history is future work).

DEFINE TABLE IF NOT EXISTS document SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON document TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON document TYPE string;
DEFINE FIELD IF NOT EXISTS title ON document TYPE string;
DEFINE FIELD IF NOT EXISTS body ON document TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS version ON document TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS created_at ON document TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON document TYPE datetime VALUE time::now();
DEFINE INDEX IF NOT EXISTS document_uuid ON document FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS document_owner ON document FIELDS owner;
