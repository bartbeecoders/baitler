//! The closed enum of host functions plugin code may call.
//!
//! Each is a thin **owner-scoped** wrapper over an existing repo: the owner
//! comes from the invocation context (never from plugin input), so a plugin
//! can never widen scope. There is NO filesystem function and no way to
//! declare one — disk simply isn't a grantable surface (`workspace.rs` stays
//! untouched). Content writes are forced to the agent tier (`review`/
//! `visibility = "draft"`) and **record their own activity row** (a host-fn
//! write bypasses `activity::entry_for`, which is keyed on MCP tool names),
//! attributed to `plugin:{slug}` plus the invoking run's `run_id`.
//!
//! All inputs/outputs are JSON strings — the lingua franca an LLM-authored
//! guest handles best. Errors return as `extism::Error`, surfacing to the
//! guest as a failed host call.
//!
//! In **test mode** (`plugins_test`, a throwaway sandbox) nothing persists:
//! `plugin_kv_*` hit an in-memory map and the content-writing functions
//! refuse, so an agent can self-verify reads/logic before the human gate
//! without writing anything.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use extism::{host_fn, Error};
use serde_json::{json, Value};

use crate::activity::{self, NewActivity};
use crate::db::Db;
use crate::ideas::model::IdeaDto;

/// Per-invocation context cloned into each registered host function.
#[derive(Clone)]
pub(super) struct HostCtx {
    pub db: Db,
    pub owner: String,
    pub slug: String,
    /// `run_id` of the invoking agent run (provenance), if any.
    pub run_id: Option<String>,
    /// Tokio runtime handle: host fns run on a blocking thread (the WASM call
    /// lives in `spawn_blocking`), so async repo calls bridge via `block_on`.
    pub handle: tokio::runtime::Handle,
    /// `Some` ⇒ throwaway `plugins_test` sandbox: kv is in-memory, writes refuse.
    pub test_kv: Option<Arc<Mutex<HashMap<String, String>>>>,
}

/// Hard ceiling on any single host-fn repo call. The WASM call runs in
/// `spawn_blocking` and host fns bridge to async repos via `block_on`; bounding
/// each call means a wedged query returns an error to the guest and RELEASES
/// the blocking-pool thread, instead of leaking it (which a route-layer timeout
/// alone cannot prevent). Generous — local/embedded queries are sub-second.
const HOST_FN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

impl HostCtx {
    fn agent_label(&self) -> String {
        format!("plugin:{}", self.slug)
    }

    /// Run a repo future to completion under [`HOST_FN_TIMEOUT`], flattening
    /// both the timeout and the app error into a guest-readable `Error`.
    fn db_call<T, F>(&self, fut: F) -> Result<T, Error>
    where
        F: std::future::Future<Output = crate::error::AppResult<T>>,
    {
        match self
            .handle
            .block_on(tokio::time::timeout(HOST_FN_TIMEOUT, fut))
        {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(Error::msg(e.to_string())),
            Err(_) => Err(Error::msg("host call timed out")),
        }
    }

    /// Record an activity row for a host-fn content write (best-effort, like
    /// the protocol layer: a provenance hiccup never fails the write itself).
    fn record_activity(&self, action: &str, target_type: &str, id: &str, title: &str) {
        let summary = format!("{action} · {title}");
        let entry = NewActivity {
            action,
            target_type,
            target_id: id,
            target_title: title,
            project_id: None,
            summary: &summary,
        };
        let agent = self.agent_label();
        if let Err(e) = self.db_call(activity::record(
            &self.db,
            &self.owner,
            Some(&agent),
            self.run_id.as_deref(),
            &entry,
        )) {
            tracing::warn!(plugin = %self.slug, error = %e, "plugin host-fn activity log failed");
        }
    }
}

fn parse_input(input: &str) -> Result<Value, Error> {
    serde_json::from_str(input).map_err(|e| Error::msg(format!("input is not JSON: {e}")))
}

fn req_str(v: &Value, key: &str) -> Result<String, Error> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| Error::msg(format!("missing or empty string field `{key}`")))
}

host_fn!(pub log(ctx: HostCtx; message: String) -> String {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().map_err(|_| Error::msg("host context poisoned"))?;
    let msg: String = message.chars().take(2_000).collect();
    tracing::info!(plugin = %ctx.slug, "{msg}");
    Ok("{}".to_string())
});

host_fn!(pub kn_search(ctx: HostCtx; input: String) -> String {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().map_err(|_| Error::msg("host context poisoned"))?;
    let v = parse_input(&input)?;
    let q = req_str(&v, "q")?;
    let limit = v.get("limit").and_then(Value::as_u64).unwrap_or(20).min(100) as usize;
    let results = ctx.db_call(crate::knowledge::repo::search(&ctx.db, &ctx.owner, &q, limit))?;
    Ok(serde_json::to_string(&results)?)
});

host_fn!(pub ideas_get(ctx: HostCtx; input: String) -> String {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().map_err(|_| Error::msg("host context poisoned"))?;
    let v = parse_input(&input)?;
    let id = req_str(&v, "id")?;
    let row = ctx.db_call(crate::ideas::repo::get_idea(&ctx.db, &ctx.owner, &id))?;
    match row {
        Some(r) => Ok(serde_json::to_string(&IdeaDto::from(r))?),
        None => Ok("null".to_string()),
    }
});

host_fn!(pub ideas_create(ctx: HostCtx; input: String) -> String {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().map_err(|_| Error::msg("host context poisoned"))?;
    if ctx.test_kv.is_some() {
        return Err(Error::msg("ideas_create is not available in the plugins_test sandbox"));
    }
    let v = parse_input(&input)?;
    let title = req_str(&v, "title")?;
    if title.len() > 200 {
        return Err(Error::msg("title longer than 200 characters"));
    }
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    let tags: Vec<String> = v
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let tags = crate::tags::normalize_tags(tags).map_err(|e| Error::msg(e.to_string()))?;
    // Forced to the agent tier: plugin writes always enter the review queue.
    let row = ctx.db_call(crate::ideas::repo::create_idea(
        &ctx.db, &ctx.owner, &title, body, &tags, "inbox", "draft", None,
    ))?;
    ctx.record_activity("idea.create", "idea", &row.uuid, &row.title);
    Ok(serde_json::to_string(&IdeaDto::from(row))?)
});

host_fn!(pub pages_create(ctx: HostCtx; input: String) -> String {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().map_err(|_| Error::msg("host context poisoned"))?;
    if ctx.test_kv.is_some() {
        return Err(Error::msg("pages_create is not available in the plugins_test sandbox"));
    }
    let v = parse_input(&input)?;
    let title = req_str(&v, "title")?;
    if title.len() > 200 {
        return Err(Error::msg("title longer than 200 characters"));
    }
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    let fmt = match v.get("source_format").and_then(Value::as_str) {
        None => "markdown",
        Some(f) if crate::pages::model::SOURCE_FORMATS.contains(&f) => f,
        Some(f) => return Err(Error::msg(format!("invalid source_format `{f}`"))),
    };
    // Forced draft visibility: a plugin can author a page, never publish one.
    let row = ctx.db_call(crate::pages::repo::create_page(
        &ctx.db, &ctx.owner, &title, body, fmt, "draft", None, None, &[],
    ))?;
    ctx.record_activity("page.create", "page", &row.uuid, &row.title);
    Ok(json!({ "id": row.uuid, "title": row.title, "slug": row.slug }).to_string())
});

host_fn!(pub plugin_kv_get(ctx: HostCtx; input: String) -> String {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().map_err(|_| Error::msg("host context poisoned"))?;
    let v = parse_input(&input)?;
    let k = req_str(&v, "k")?;
    let stored = if let Some(test_kv) = &ctx.test_kv {
        test_kv.lock().map_err(|_| Error::msg("test kv poisoned"))?.get(&k).cloned()
    } else {
        ctx.db_call(super::repo::kv_get(&ctx.db, &ctx.owner, &ctx.slug, &k))?
    };
    // The stored value is JSON; embed it raw so the guest reads `{"v": …}`.
    Ok(match stored {
        Some(raw) => format!("{{\"v\":{raw}}}"),
        None => "{\"v\":null}".to_string(),
    })
});

host_fn!(pub plugin_kv_set(ctx: HostCtx; input: String) -> String {
    let ctx = ctx.get()?;
    let ctx = ctx.lock().map_err(|_| Error::msg("host context poisoned"))?;
    let v = parse_input(&input)?;
    let k = req_str(&v, "k")?;
    let value = v.get("v").cloned().unwrap_or(Value::Null);
    let raw = serde_json::to_string(&value)?;
    if let Some(test_kv) = &ctx.test_kv {
        test_kv.lock().map_err(|_| Error::msg("test kv poisoned"))?.insert(k, raw);
    } else {
        ctx.db_call(super::repo::kv_set(&ctx.db, &ctx.owner, &ctx.slug, &k, &raw))?;
    }
    Ok("{}".to_string())
});
