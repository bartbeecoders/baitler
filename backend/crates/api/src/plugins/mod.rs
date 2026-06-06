//! Plugin system (Phase 16): agent-authored, human-approved extensions.
//!
//! Phase 16.1 lands only the **registry seam**: a [`PluginRegistry`] hangs off
//! [`crate::state::AppState`], the MCP dispatcher routes `plugin__*` names to
//! it, and `tools/list` chains [`PluginRegistry::tool_defs`] after the static
//! catalog. Nothing populates the registry yet — the storage/lifecycle layer,
//! the WASM runtime (Extism, behind the `plugins` Cargo feature), and the
//! `plugins_*` meta-tools land in Phase 16.B — so observable behaviour is
//! unchanged: every `plugin__*` call resolves to the same unknown-tool error
//! the static dispatcher returned before this seam existed.

mod registry;

pub use registry::{LoadedPlugin, PluginRegistry};

/// Namespace prefix for plugin-provided MCP tools: `plugin__{slug}__{tool}`.
///
/// Namespacing keeps dynamic tools from ever colliding with (or shadowing) a
/// static tool; the drift-guard test asserts no static name uses this prefix.
pub const TOOL_PREFIX: &str = "plugin__";
