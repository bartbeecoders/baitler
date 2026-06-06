//! Runtime registry of enabled plugins.
//!
//! Process-local, behind a `std::sync::RwLock` with short critical sections
//! never held across an `await` (reads dominate: every `tools/list` and every
//! `plugin__*` dispatch; writes happen only on enable/disable/uninstall/
//! startup). Populated in `build_state` from `status = "enabled"` rows and by
//! the portal enable/disable REST handlers.
//!
//! Loading **re-verifies the pinned `bundle_sha256`** (a re-authored artifact
//! fails the digest and must re-enter review) and **refuses a stale
//! `api_version`** (a platform ABI bump auto-disables old plugins instead of
//! silently breaking them). Execution happens in `runtime.rs` behind the
//! `plugins` Cargo feature; without it, dispatch reports the runtime as
//! unavailable while the whole authoring/lifecycle surface keeps working.

use std::sync::{Arc, RwLock};

use serde_json::{json, Value};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::mcp::tools::ToolError;
use crate::mcp::Actor;
use crate::state::AppState;

use super::manifest::{Manifest, HOST_API_VERSION};
use super::model::PluginRow;
use super::repo;
use super::TOOL_PREFIX;

/// One enabled plugin, loaded and pinned: parsed manifest, verified WASM
/// bytes, and the precomputed wire-shape MCP tool definitions.
pub struct LoadedPlugin {
    pub uuid: String,
    pub owner: String,
    pub slug: String,
    pub manifest: Manifest,
    pub wasm: Vec<u8>,
    pub tool_defs: Vec<Value>,
}

/// Registry of loaded plugins, hung off [`crate::state::AppState`].
#[derive(Default)]
pub struct PluginRegistry {
    plugins: RwLock<Vec<Arc<LoadedPlugin>>>,
}

impl PluginRegistry {
    /// Load (or replace) one enabled plugin from its row: parse the manifest,
    /// check the ABI epoch, fetch + digest-verify the WASM, precompute tool
    /// definitions. Errors describe what the portal should surface.
    pub async fn load(&self, db: &Db, row: &PluginRow) -> AppResult<()> {
        let parsed: Manifest = serde_json::from_str(&row.manifest).map_err(|e| {
            AppError::Internal(format!("stored plugin manifest is unreadable: {e}").into())
        })?;
        if parsed.api_version != HOST_API_VERSION {
            return Err(AppError::Unavailable(format!(
                "plugin `{}` was built against plugin ABI v{} (host is v{HOST_API_VERSION}); \
                 re-create it against the current API",
                row.slug, parsed.api_version
            )));
        }
        let wasm_b64 = repo::get_wasm_b64(db, &row.owner, &row.uuid)
            .await?
            .ok_or(AppError::NotFound)?;
        let wasm = if wasm_b64.is_empty() {
            Vec::new()
        } else {
            crate::mcp::b64::decode(&wasm_b64)
                .map_err(|e| AppError::Internal(format!("stored wasm is not Base64: {e}").into()))?
        };
        let ui_assets_json = repo::get_ui_assets_json(db, &row.owner, &row.uuid)
            .await?
            .unwrap_or_default();
        let digest = repo::bundle_sha256(&row.manifest, &wasm, &ui_assets_json);
        if digest != row.bundle_sha256 {
            return Err(AppError::Conflict(format!(
                "plugin `{}` bundle digest mismatch (stored {}, computed {digest}) — \
                 the artifact changed since install; re-create the plugin so it \
                 re-enters review",
                row.slug, row.bundle_sha256
            )));
        }
        let tool_defs = wire_tool_defs(&row.slug, &parsed);
        let loaded = Arc::new(LoadedPlugin {
            uuid: row.uuid.clone(),
            owner: row.owner.clone(),
            slug: row.slug.clone(),
            manifest: parsed,
            wasm,
            tool_defs,
        });
        let mut plugins = self.plugins.write().expect("plugin registry lock");
        plugins.retain(|p| !(p.owner == loaded.owner && p.slug == loaded.slug));
        plugins.push(loaded);
        Ok(())
    }

    /// Remove a plugin from the registry (disable/uninstall/replace).
    pub fn unload(&self, owner: &str, slug: &str) {
        self.plugins
            .write()
            .expect("plugin registry lock")
            .retain(|p| !(p.owner == owner && p.slug == slug));
    }

    /// Loaded plugin count (startup logging / status surfaces).
    pub fn len(&self) -> usize {
        self.plugins.read().expect("plugin registry lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

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

    /// Resolve a `plugin__{slug}__{tool}` name to the loaded plugin and the
    /// declared tool's WASM export. `None` ⇒ unknown tool.
    fn resolve(&self, name: &str) -> Option<(Arc<LoadedPlugin>, String)> {
        let rest = name.strip_prefix(TOOL_PREFIX)?;
        // Slugs are [a-z0-9-] and tool names forbid `__` runs, so the first
        // `__` is an unambiguous separator.
        let (slug, tool) = rest.split_once("__")?;
        let plugins = self.plugins.read().expect("plugin registry lock");
        let plugin = plugins.iter().find(|p| p.slug == slug)?.clone();
        drop(plugins);
        let export = plugin
            .manifest
            .tools
            .iter()
            .find(|t| t.name == tool)?
            .export
            .clone();
        Some((plugin, export))
    }

    /// Execute a plugin-provided tool (a [`TOOL_PREFIX`]ed name).
    ///
    /// The full [`Actor`] threads through — not just the owner — so the
    /// invocation carries provenance (`agent`/`run_id`) into host-fn activity
    /// rows. Owner scope is re-derived from the actor: a loaded plugin owned by
    /// someone else is invisible (multi-owner readiness).
    pub(crate) async fn dispatch(
        &self,
        state: &AppState,
        actor: &Actor,
        name: &str,
        args: &Value,
    ) -> Result<Value, ToolError> {
        let Some((plugin, export)) = self.resolve(name) else {
            return Err(ToolError::UnknownTool);
        };
        if plugin.owner != actor.owner {
            return Err(ToolError::UnknownTool);
        }
        invoke(state, actor, &plugin, &export, args).await
    }

    /// Invoke a tool by `(slug, tool)` — the `plugins_invoke` meta-tool path.
    pub(crate) async fn invoke_tool(
        &self,
        state: &AppState,
        actor: &Actor,
        slug: &str,
        tool: &str,
        args: &Value,
    ) -> Result<Value, ToolError> {
        self.dispatch(state, actor, &format!("{TOOL_PREFIX}{slug}__{tool}"), args)
            .await
    }

    /// Resolve an enabled plugin's declared HTTP endpoint (`method` + `path`
    /// under its mount) to the loaded plugin and the bound WASM export — the
    /// Phase 16.C endpoint kind. `None` ⇒ no such plugin/endpoint (404).
    pub(crate) fn resolve_endpoint(
        &self,
        owner: &str,
        slug: &str,
        method: &str,
        path: &str,
    ) -> Option<(Arc<LoadedPlugin>, String)> {
        let plugins = self.plugins.read().expect("plugin registry lock");
        let plugin = plugins
            .iter()
            .find(|p| p.slug == slug && p.owner == owner)?
            .clone();
        drop(plugins);
        let export = plugin
            .manifest
            .endpoints
            .iter()
            .find(|e| e.method == method && e.path == path)?
            .export
            .clone();
        Some((plugin, export))
    }

    /// Run a resolved export directly (the endpoint-dispatch path; tools go
    /// through [`Self::dispatch`]). Same feature-gated runtime underneath.
    pub(crate) async fn invoke_export(
        &self,
        state: &AppState,
        actor: &Actor,
        plugin: &Arc<LoadedPlugin>,
        export: &str,
        args: &Value,
    ) -> Result<Value, ToolError> {
        invoke(state, actor, plugin, export, args).await
    }
}

/// Wire-shape MCP definitions for a plugin's tools, namespaced and tagged with
/// provenance so a model knows it is calling an installed plugin.
fn wire_tool_defs(slug: &str, m: &Manifest) -> Vec<Value> {
    m.tools
        .iter()
        .map(|t| {
            json!({
                "name": format!("{TOOL_PREFIX}{slug}__{}", t.name),
                "description": format!(
                    "[plugin {slug} v{}] {}",
                    m.version,
                    if t.description.is_empty() { &m.description } else { &t.description }
                ),
                "inputSchema": t.input_schema,
            })
        })
        .collect()
}

/// Run one export of a loaded plugin (feature-gated runtime).
#[cfg(feature = "plugins")]
async fn invoke(
    state: &AppState,
    actor: &Actor,
    plugin: &Arc<LoadedPlugin>,
    export: &str,
    args: &Value,
) -> Result<Value, ToolError> {
    super::runtime::invoke(state, actor, plugin, export, args)
        .await
        .map_err(ToolError::App)
}

/// Without the `plugins` feature the lifecycle works but nothing executes.
#[cfg(not(feature = "plugins"))]
async fn invoke(
    _state: &AppState,
    _actor: &Actor,
    _plugin: &Arc<LoadedPlugin>,
    _export: &str,
    _args: &Value,
) -> Result<Value, ToolError> {
    Err(ToolError::Invalid(
        "the plugin runtime is not compiled into this build — rebuild the backend with \
         `--features plugins` to execute plugin code"
            .into(),
    ))
}

/// Load every `status = "enabled"` plugin for the dev owner at startup,
/// auto-disabling any that no longer load (digest mismatch / stale ABI) so a
/// platform upgrade degrades loudly-but-gracefully instead of crashing or
/// silently breaking. Called from `build_state` after migrations.
pub async fn load_enabled(state: &AppState) {
    if !state.config.plugins.enabled {
        return;
    }
    let owner = crate::owner::DEV_OWNER;
    let rows = match repo::enabled_plugins(&state.db, owner).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "plugin registry: listing enabled plugins failed");
            return;
        }
    };
    for row in rows {
        if let Err(e) = state.plugins.load(&state.db, &row).await {
            tracing::warn!(slug = %row.slug, error = %e, "plugin failed to load — disabling");
            if let Err(e2) = repo::set_status(&state.db, owner, &row.uuid, "disabled").await {
                tracing::warn!(slug = %row.slug, error = %e2, "auto-disable failed");
            }
        }
    }
    let n = state.plugins.len();
    if n > 0 {
        tracing::info!(count = n, "plugin registry: enabled plugins loaded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owner::DEV_OWNER;

    fn loaded(slug: &str, tool: &str) -> Arc<LoadedPlugin> {
        let manifest: Manifest = serde_json::from_value(json!({
            "name": slug,
            "tools": [{ "name": tool, "export": "run" }]
        }))
        .unwrap();
        let tool_defs = wire_tool_defs(slug, &manifest);
        Arc::new(LoadedPlugin {
            uuid: "u1".into(),
            owner: DEV_OWNER.into(),
            slug: slug.into(),
            manifest,
            wasm: Vec::new(),
            tool_defs,
        })
    }

    #[test]
    fn empty_registry_contributes_no_tools() {
        assert!(PluginRegistry::default().tool_defs().is_empty());
    }

    #[test]
    fn resolve_parses_namespaced_names() {
        let reg = PluginRegistry::default();
        reg.plugins
            .write()
            .unwrap()
            .push(loaded("csv-stats", "summarize"));

        let (p, export) = reg.resolve("plugin__csv-stats__summarize").expect("hit");
        assert_eq!(p.slug, "csv-stats");
        assert_eq!(export, "run");

        assert!(reg.resolve("plugin__csv-stats__nope").is_none());
        assert!(reg.resolve("plugin__other__summarize").is_none());
        assert!(reg.resolve("plugin__csv-stats").is_none());
        assert!(reg.resolve("ideas_list").is_none());
    }

    #[test]
    fn wire_defs_are_namespaced_with_provenance() {
        let defs = loaded("csv-stats", "summarize").tool_defs.clone();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["name"], "plugin__csv-stats__summarize");
        assert!(defs[0]["description"]
            .as_str()
            .unwrap()
            .starts_with("[plugin csv-stats v0.1.0]"));
        assert_eq!(defs[0]["inputSchema"]["type"], "object");
    }

    #[test]
    fn unload_removes_by_owner_and_slug() {
        let reg = PluginRegistry::default();
        reg.plugins
            .write()
            .unwrap()
            .push(loaded("csv-stats", "summarize"));
        assert_eq!(reg.len(), 1);
        reg.unload("someone-else", "csv-stats");
        assert_eq!(reg.len(), 1);
        reg.unload(DEV_OWNER, "csv-stats");
        assert!(reg.is_empty());
    }

    #[test]
    fn resolve_misses_on_empty_registry() {
        // dispatch() needs an AppState (DB) only after resolve() hits, so the
        // unknown-tool path is fully covered by resolve() returning None; the
        // full dispatch path is exercised by the integration tests.
        assert!(PluginRegistry::default()
            .resolve("plugin__nope__tool")
            .is_none());
    }
}
