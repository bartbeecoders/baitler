-- 0006_knowledge — projects, cross-type links, and the draft/published gate.
--
-- Two additive primitives layered onto the existing owner-scoped model:
--   * `project` groups files/ideas/documents for "a project I'm working on".
--     Membership is the direct `project_id` pointer on each item (one project per
--     item — the common case), so listing a project's members is a single indexed
--     query, no edge bookkeeping.
--   * `kn_link` is a polymorphic cross-type LINK table (idea↔document↔file↔project).
--     Links are app-maintained (no graph relations, to fit the uuid-public-id
--     model) and written as BOTH directed rows so the `kn_link_dst` index always
--     applies and backlinks resolve from either endpoint — mirroring the Phase 5
--     symmetric `link_ideas` precedent.
--
-- Also adds the `review` gate (draft|published) to idea+document. Existing rows are
-- backfilled to "published" so nothing already captured is hidden.

-- ── project ────────────────────────────────────────────────────────────────
DEFINE TABLE IF NOT EXISTS project SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON project TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON project TYPE string;
DEFINE FIELD IF NOT EXISTS name ON project TYPE string;
DEFINE FIELD IF NOT EXISTS slug ON project TYPE string;
DEFINE FIELD IF NOT EXISTS summary ON project TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS status ON project TYPE string DEFAULT "active";
DEFINE FIELD IF NOT EXISTS created_at ON project TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON project TYPE datetime VALUE time::now();
DEFINE INDEX IF NOT EXISTS project_uuid ON project FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS project_slug ON project FIELDS owner, slug UNIQUE;
DEFINE INDEX IF NOT EXISTS project_owner ON project FIELDS owner, status;

-- ── kn_link (symmetric cross-type links) ───────────────────────────────────
DEFINE TABLE IF NOT EXISTS kn_link SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON kn_link TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON kn_link TYPE string;
DEFINE FIELD IF NOT EXISTS src_type ON kn_link TYPE string;               -- idea|document|file|project
DEFINE FIELD IF NOT EXISTS src_id ON kn_link TYPE string;
DEFINE FIELD IF NOT EXISTS dst_type ON kn_link TYPE string;
DEFINE FIELD IF NOT EXISTS dst_id ON kn_link TYPE string;
DEFINE FIELD IF NOT EXISTS relation ON kn_link TYPE string DEFAULT "";    -- free-form: contains|implements|...
DEFINE FIELD IF NOT EXISTS created_at ON kn_link TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS kn_link_uuid ON kn_link FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS kn_link_owner ON kn_link FIELDS owner;
DEFINE INDEX IF NOT EXISTS kn_link_src ON kn_link FIELDS owner, src_type, src_id;
DEFINE INDEX IF NOT EXISTS kn_link_dst ON kn_link FIELDS owner, dst_type, dst_id;

-- ── review gate + project membership pointer on existing tables ────────────
DEFINE FIELD IF NOT EXISTS review ON idea TYPE string DEFAULT "published";
DEFINE FIELD IF NOT EXISTS project_id ON idea TYPE option<string> DEFAULT NONE;
DEFINE INDEX IF NOT EXISTS idea_review ON idea FIELDS owner, review;
DEFINE INDEX IF NOT EXISTS idea_project ON idea FIELDS owner, project_id;

DEFINE FIELD IF NOT EXISTS review ON document TYPE string DEFAULT "published";
DEFINE FIELD IF NOT EXISTS project_id ON document TYPE option<string> DEFAULT NONE;
DEFINE INDEX IF NOT EXISTS document_review ON document FIELDS owner, review;
DEFINE INDEX IF NOT EXISTS document_project ON document FIELDS owner, project_id;

DEFINE FIELD IF NOT EXISTS project_id ON file TYPE option<string> DEFAULT NONE;
DEFINE INDEX IF NOT EXISTS file_project ON file FIELDS owner, project_id;

-- Backfill: any pre-existing row predates the gate → treat as already published.
UPDATE idea SET review = "published" WHERE review = NONE;
UPDATE document SET review = "published" WHERE review = NONE;
