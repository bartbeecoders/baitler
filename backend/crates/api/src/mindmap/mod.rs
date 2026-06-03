//! Mindmaps (Phase 14, Milestone B): a node/edge graph content type shaped like
//! a `document` (owner/title/folder_id/project_id/tags/review/version + a typed
//! JSON body), so it reuses folders, projects, `kn_link`, `knowledge_search`,
//! the draft gate, activity, and export with no bespoke graph schema. Authored
//! freehand, from a Markdown outline, or seeded from a project's ideas + links.

pub mod model;
pub mod repo;
pub mod routes;
