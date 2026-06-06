//! Plugin system (Phase 16): agent-authored, human-approved extensions.
//!
//! The loop this module exists for: an agent scaffolds + implements + tests a
//! plugin entirely over `/mcp` (`plugins_*` meta-tools), submits it as a
//! **forced draft**, a human inspects the capability manifest and approves +
//! enables it in the portal (REST-only — there is deliberately NO MCP verb for
//! approve/enable), and the plugin's tools go live as `plugin__{slug}__{tool}`
//! for every future session.
//!
//! Layout: [`manifest`] (deny-unknown-fields declaration + default-deny
//! capabilities), [`model`]/[`repo`] (storage; forced-draft invariant),
//! [`registry`] (the runtime registry hung off `AppState`), and — behind the
//! `plugins` Cargo feature — `runtime`/`hostfns` (the Extism sandbox and the
//! closed enum of owner-scoped host functions). Without the feature the whole
//! lifecycle works but execution paths report the runtime as unavailable.

pub mod manifest;
pub mod model;
pub mod public;
mod registry;
pub mod repo;
pub mod routes;

#[cfg(feature = "plugins")]
mod hostfns;
#[cfg(feature = "plugins")]
mod runtime;

pub use registry::{load_enabled, LoadedPlugin, PluginRegistry};

/// Run one export in a **throwaway** sandbox (`plugins_test`): same runtime
/// and grants as a real invocation, but nothing persists and nothing touches
/// the registry. Builds without the `plugins` feature report the runtime as
/// unavailable instead.
#[cfg(feature = "plugins")]
pub(crate) async fn test_run(
    state: &crate::state::AppState,
    actor: &crate::mcp::Actor,
    manifest: &manifest::Manifest,
    wasm: Vec<u8>,
    export: &str,
    input: &serde_json::Value,
) -> crate::error::AppResult<serde_json::Value> {
    runtime::test_run(state, actor, manifest, wasm, export, input).await
}

#[cfg(not(feature = "plugins"))]
pub(crate) async fn test_run(
    _state: &crate::state::AppState,
    _actor: &crate::mcp::Actor,
    _manifest: &manifest::Manifest,
    _wasm: Vec<u8>,
    _export: &str,
    _input: &serde_json::Value,
) -> crate::error::AppResult<serde_json::Value> {
    Err(crate::error::AppError::Unavailable(
        "the plugin runtime is not compiled into this build — rebuild the backend with \
         `--features plugins` to execute plugin code"
            .into(),
    ))
}

/// Namespace prefix for plugin-provided MCP tools: `plugin__{slug}__{tool}`.
///
/// Namespacing keeps dynamic tools from ever colliding with (or shadowing) a
/// static tool; the drift-guard test asserts no static name uses this prefix.
pub const TOOL_PREFIX: &str = "plugin__";
