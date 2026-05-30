//! Knowledge layer (Phase 11): projects that group ideas/documents/files, and a
//! polymorphic cross-type link graph (`kn_link`). Membership is the direct
//! `project_id` pointer on each item; links are symmetric `kn_link` edge rows.
//! Owner-scoped throughout, like every other feature.

pub mod model;
pub mod repo;
