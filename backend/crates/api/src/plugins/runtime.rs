//! Extism (Wasmtime) execution of plugin WASM — the codebase's untrusted-code
//! boundary, compiled only with the `plugins` Cargo feature.
//!
//! Containment, per invocation:
//! - a **fresh instance** every call (no state leaks between invocations;
//!   persistence is exclusively the `plugin_kv` host fns);
//! - **only the granted host functions are registered**, so a guest importing
//!   anything else fails at instantiation — the capability check is structural,
//!   not a runtime branch;
//! - `memory_max_pages` caps guest memory and `timeout_ms` arms Extism's
//!   epoch-interruption kill, both from the (validated, ceiling-checked)
//!   manifest;
//! - no WASI, no filesystem, no network: the Extism manifest gets no
//!   `allowed_paths`/`allowed_hosts` — those knobs are never even plumbed.
//!
//! The blocking WASM call runs in `spawn_blocking`; host fns bridge back to
//! async repos via the captured runtime `Handle` (safe on a blocking thread).

use std::sync::Arc;
use std::time::Duration;

use extism::{Manifest as WasmManifest, PluginBuilder, UserData, Wasm, PTR};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::mcp::Actor;
use crate::state::AppState;

use super::hostfns::{self, HostCtx};
use super::manifest::Manifest;
use super::registry::LoadedPlugin;

/// Outcome of one export call (also the `plugins_test` report shape).
pub(super) struct RunOutcome {
    pub output: Value,
    pub ms: u128,
    pub limits_hit: bool,
}

/// Execute one export of an **enabled, registry-loaded** plugin.
pub(super) async fn invoke(
    state: &AppState,
    actor: &Actor,
    plugin: &Arc<LoadedPlugin>,
    export: &str,
    args: &Value,
) -> AppResult<Value> {
    if !state.config.plugins.enabled {
        return Err(AppError::Unavailable(
            "the plugin system is disabled (set PLUGINS_ENABLED=true)".into(),
        ));
    }
    let ctx = HostCtx {
        db: state.db.clone(),
        owner: actor.owner.clone(),
        slug: plugin.slug.clone(),
        run_id: actor.run_id.clone(),
        handle: tokio::runtime::Handle::current(),
        test_kv: None,
    };
    let outcome = run(
        plugin.wasm.clone(),
        plugin.manifest.clone(),
        ctx,
        export.to_string(),
        args.clone(),
    )
    .await
    .map_err(|msg| AppError::BadRequest(format!("plugin `{}` failed: {msg}", plugin.slug)))?;
    tracing::info!(
        plugin = %plugin.slug,
        export,
        ms = outcome.ms as u64,
        limits_hit = outcome.limits_hit,
        "plugin invocation"
    );
    Ok(outcome.output)
}

/// Execute one export in a **throwaway sandbox** (`plugins_test`): same
/// runtime, same grants — but nothing persists (in-memory kv, content writes
/// refused) and nothing touches the registry or the row store.
pub(super) async fn test_run(
    state: &AppState,
    actor: &Actor,
    manifest: &Manifest,
    wasm: Vec<u8>,
    export: &str,
    input: &Value,
) -> AppResult<Value> {
    if !state.config.plugins.enabled {
        return Err(AppError::Unavailable(
            "the plugin system is disabled (set PLUGINS_ENABLED=true)".into(),
        ));
    }
    let ctx = HostCtx {
        db: state.db.clone(),
        owner: actor.owner.clone(),
        slug: crate::slug::slugify(&manifest.name, "plugin"),
        run_id: actor.run_id.clone(),
        handle: tokio::runtime::Handle::current(),
        test_kv: Some(Default::default()),
    };
    let started = std::time::Instant::now();
    let result = run(
        wasm,
        manifest.clone(),
        ctx,
        export.to_string(),
        input.clone(),
    )
    .await;
    let ms = started.elapsed().as_millis();
    Ok(match result {
        Ok(outcome) => json!({
            "ok": true,
            "output": outcome.output,
            "ms": outcome.ms as u64,
            "limits_hit": outcome.limits_hit,
        }),
        // Test failures are DATA for the authoring agent, not tool errors.
        Err(msg) => json!({
            "ok": false,
            "error": msg.clone(),
            "ms": ms as u64,
            "limits_hit": looks_like_limit(&msg),
        }),
    })
}

/// Build a fresh sandbox and call `export`. String errors are guest/limit
/// failures (the caller decides whether they are tool errors or test data).
async fn run(
    wasm: Vec<u8>,
    manifest: Manifest,
    ctx: HostCtx,
    export: String,
    args: Value,
) -> Result<RunOutcome, String> {
    if wasm.is_empty() {
        return Err("plugin has no WASM module (UI-only plugins execute nothing)".into());
    }
    let input = args.to_string();
    tokio::task::spawn_blocking(move || {
        let wasm_manifest = WasmManifest::new([Wasm::data(wasm)])
            .with_memory_max(manifest.capabilities.memory_max_pages)
            .with_timeout(Duration::from_millis(manifest.capabilities.timeout_ms));

        let mut builder = PluginBuilder::new(wasm_manifest).with_wasi(false);
        // Register ONLY the granted host fns: an ungranted import fails
        // instantiation (structural containment).
        for grant in &manifest.capabilities.host_fns {
            builder = match grant.as_str() {
                "log" => builder.with_function(
                    "log",
                    [PTR],
                    [PTR],
                    UserData::new(ctx.clone()),
                    hostfns::log,
                ),
                "kn_search" => builder.with_function(
                    "kn_search",
                    [PTR],
                    [PTR],
                    UserData::new(ctx.clone()),
                    hostfns::kn_search,
                ),
                "ideas_get" => builder.with_function(
                    "ideas_get",
                    [PTR],
                    [PTR],
                    UserData::new(ctx.clone()),
                    hostfns::ideas_get,
                ),
                "ideas_create" => builder.with_function(
                    "ideas_create",
                    [PTR],
                    [PTR],
                    UserData::new(ctx.clone()),
                    hostfns::ideas_create,
                ),
                "pages_create" => builder.with_function(
                    "pages_create",
                    [PTR],
                    [PTR],
                    UserData::new(ctx.clone()),
                    hostfns::pages_create,
                ),
                "plugin_kv_get" => builder.with_function(
                    "plugin_kv_get",
                    [PTR],
                    [PTR],
                    UserData::new(ctx.clone()),
                    hostfns::plugin_kv_get,
                ),
                "plugin_kv_set" => builder.with_function(
                    "plugin_kv_set",
                    [PTR],
                    [PTR],
                    UserData::new(ctx.clone()),
                    hostfns::plugin_kv_set,
                ),
                // Unreachable: manifests are validated against the closed enum
                // before storage, and the registry re-parses on load.
                other => return Err(format!("unknown host fn grant `{other}`")),
            };
        }
        let mut plugin = builder
            .build()
            .map_err(|e| format!("instantiation failed: {e}"))?;

        let started = std::time::Instant::now();
        let output = plugin.call::<&str, String>(&export, &input);
        let ms = started.elapsed().as_millis();
        match output {
            Ok(raw) => {
                let output = serde_json::from_str(&raw).unwrap_or_else(|_| Value::String(raw));
                Ok(RunOutcome {
                    output,
                    ms,
                    limits_hit: false,
                })
            }
            Err(e) => {
                let msg = e.to_string();
                if looks_like_limit(&msg) {
                    Err(format!(
                        "killed by resource limit after {ms}ms ({msg}); raise \
                         capabilities.timeout_ms / memory_max_pages only as far as the ceiling"
                    ))
                } else {
                    // The export must match BOTH a symbol the WASM module
                    // actually exports AND a tools[].export in the manifest —
                    // list the declared ones so a blind-iterating agent
                    // one-shots the fix instead of guessing.
                    let declared: Vec<&str> =
                        manifest.tools.iter().map(|t| t.export.as_str()).collect();
                    Err(format!(
                        "call `{export}` failed: {msg} (manifest declares exports: {declared:?})"
                    ))
                }
            }
        }
    })
    .await
    .map_err(|e| format!("plugin task panicked: {e}"))?
}

/// Did the guest die on a resource limit? Anchored to the exact strings Extism
/// 1.x surfaces for the epoch-timeout and memory kills (`"timeout"` / `"oom"`)
/// rather than a substring scan — guest trap text is attacker-authored, and a
/// loose match would mislabel ordinary failures (e.g. "out of memory in my
/// parser") as host-imposed limit kills with a bogus remediation hint.
fn looks_like_limit(msg: &str) -> bool {
    let m = msg.trim().to_ascii_lowercase();
    m == "timeout" || m == "oom"
}
