//! Hosted web pages (Phase 12): document-like, slug-addressed, visibility-gated
//! artifacts that can be published to a served URL (`GET /p/{slug}`). Authored
//! through the shared `convert.rs` pathway, filed in the Phase 4 `folder` tree,
//! and joinable to Phase 11 projects/links. Owner-scoped management; the public
//! serve path is unauthenticated by design (see `public`, Phase 12.5).

pub mod model;
pub mod repo;
pub mod routes;
