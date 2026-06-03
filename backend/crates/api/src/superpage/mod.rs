//! Superpages (Phase 15): a block-composition canvas that references existing
//! Baitler content on one board. Shaped like `mindmap`/`document` (metadata +
//! JSON body) so folders, projects, `kn_link`, search, review, activity, and MCP
//! reuse the same cross-cutting machinery.

pub mod context;
pub mod model;
pub mod repo;
pub mod routes;