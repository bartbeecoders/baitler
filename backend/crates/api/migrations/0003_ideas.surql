-- 0003_ideas — idea/note management.
--
-- Public id is `uuid` (server-generated). `links` holds the uuids of related
-- ideas and is kept symmetric (undirected) by the application. `status` drives
-- the board view (inbox/active/done/archived). Owner-scoped like files.

DEFINE TABLE IF NOT EXISTS idea SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON idea TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON idea TYPE string;
DEFINE FIELD IF NOT EXISTS title ON idea TYPE string;
DEFINE FIELD IF NOT EXISTS body ON idea TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS tags ON idea TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS status ON idea TYPE string DEFAULT "inbox";
DEFINE FIELD IF NOT EXISTS links ON idea TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS created_at ON idea TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON idea TYPE datetime VALUE time::now();
DEFINE INDEX IF NOT EXISTS idea_uuid ON idea FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS idea_owner ON idea FIELDS owner, status;
