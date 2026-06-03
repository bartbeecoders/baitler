//! draw.io diagrams (Phase 14, Milestone C): a content type shaped like a
//! `document` (owner/title/folder_id/project_id/tags/review/version + an mxGraph
//! XML body), so it reuses folders, projects, `kn_link`, `knowledge_search`, the
//! draft gate, activity, and export with no bespoke schema. Authored in the
//! embedded draw.io editor; a sanitized SVG/PNG preview is rendered as a static
//! thumbnail everywhere outside the live editor.

pub mod model;
pub mod repo;
pub mod routes;
