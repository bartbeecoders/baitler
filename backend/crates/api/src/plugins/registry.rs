//! Runtime registry of enabled plugins.
//!
//! Process-local, like [`crate::cli::RunRegistry`]: plugins behind a
//! `std::sync` lock with short critical sections that are never held across an
//! `await`. Reads dominate (every `tools/list` and every `plugin__*` dispatch);
//! writes happen only on enable/disable/uninstall, so this is an `RwLock`.

use std::sync::{Arc, RwLock};

use serde_json::Value;

use crate::mcp::tools::ToolError;
use crate::mcp::Actor;

/// One enabled plugin, as loaded from its `plugin` row.
///
/// Phase 16.B extends this with the validated manifest, capability grants, and
/// the instantiated WASM module; the seam only needs identity plus the tools
/// the plugin advertises.
pub struct LoadedPlugin {
    /// Owner-scoped slug; tools advertise as `plugin__{slug}__{tool}`.
    pub slug: String,
    /// Wire-shape MCP tool definitions this plugin contributes to `tools/list`.
    pub tool_defs: Vec<Value>,
}

/// Registry of loaded plugins, hung off [`crate::state::AppState`].
///
/// Constructed empty; the Phase 16.B loader populates it from
/// `status = "enabled"` rows in `build_state` (after migrations, next to
/// `fail_orphaned_runs`) and on portal enable/disable.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: RwLock<Vec<Arc<LoadedPlugin>>>,
}

impl PluginRegistry {
    /// Tool definitions contributed by every loaded plugin, in wire shape —
    /// chained after the static catalog by [`crate::mcp::tools::definitions`].
    pub fn tool_defs(&self) -> Vec<Value> {
        self.plugins
            .read()
            .expect("plugin registry lock")
            .iter()
            .flat_map(|p| p.tool_defs.iter().cloned())
            .collect()
    }

    /// Execute a plugin-provided tool (a [`TOOL_PREFIX`]ed name).
    ///
    /// The full [`Actor`] is threaded through — not just the owner — so plugin
    /// invocations carry provenance (`agent`/`run_id`) and Phase 16.B can gate
    /// restricted runs at this one choke point. Until the 16.B runtime lands
    /// nothing populates the registry, so every name resolves to the same
    /// [`ToolError::UnknownTool`] the static dispatcher returned before the
    /// seam existed.
    ///
    /// [`TOOL_PREFIX`]: super::TOOL_PREFIX
    pub(crate) async fn dispatch(
        &self,
        _actor: &Actor,
        _name: &str,
        _args: &Value,
    ) -> Result<Value, ToolError> {
        Err(ToolError::UnknownTool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owner::DEV_OWNER;

    #[test]
    fn empty_registry_contributes_no_tools() {
        assert!(PluginRegistry::default().tool_defs().is_empty());
    }

    #[tokio::test]
    async fn empty_registry_dispatch_is_unknown_tool() {
        let actor = Actor {
            owner: DEV_OWNER.to_string(),
            agent: None,
            run_id: None,
        };
        let result = PluginRegistry::default()
            .dispatch(&actor, "plugin__nope__tool", &Value::Null)
            .await;
        assert!(matches!(result, Err(ToolError::UnknownTool)));
    }
}
