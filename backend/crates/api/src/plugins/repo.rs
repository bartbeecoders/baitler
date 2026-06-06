//! SurrealDB persistence for plugins. Owner-scoped throughout.
//!
//! The load-bearing invariant lives here: **every write lands as
//! `review = "draft"` AND `status = "draft"`, unconditionally** — the caller
//! cannot pass a review state (unlike `clean_review`-gated content types, which
//! accept `published`). The only way out of draft is the portal-only lifecycle
//! REST (`approve`/`enable`/`disable`/`reject`); no MCP verb can transition
//! status. `wasm_b64` is excluded from the standard projection (it can be MBs);
//! [`get_wasm_b64`] fetches it for registry load / export only.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::slug;

use super::manifest::Manifest;
use super::model::{PluginRow, PLUGIN_STATUSES};

/// Standard projection — deliberately WITHOUT `wasm_b64`.
const PLUGIN_SELECT: &str = "SELECT uuid, owner, name, slug, description, version, version_int, \
    api_version, manifest, kinds, status, review, bundle_sha256, project_id, author_agent, \
    run_id, type::string(created_at) AS created_at, type::string(updated_at) AS updated_at \
    FROM plugin";

/// Cap a stored KV value (the only plugin persistence).
const MAX_KV_VALUE: usize = 64 * 1024;
/// Cap distinct keys per plugin.
const MAX_KV_KEYS: usize = 1_000;

fn vanished() -> AppError {
    AppError::Internal("plugin not found immediately after write".into())
}

/// Content hash pinning the installed artifact: manifest JSON + WASM bytes.
/// Re-checked by the registry on every load; a mismatch refuses to run.
pub fn bundle_sha256(manifest_json: &str, wasm: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(manifest_json.as_bytes());
    h.update(wasm);
    hex::encode(h.finalize())
}

pub async fn get_plugin(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<PluginRow>> {
    let sql = format!("{PLUGIN_SELECT} WHERE owner = $owner AND uuid = $uuid");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<PluginRow>>(0)?.into_iter().next())
}

pub async fn get_by_slug(db: &Db, owner: &str, slug: &str) -> AppResult<Option<PluginRow>> {
    let sql = format!("{PLUGIN_SELECT} WHERE owner = $owner AND slug = $slug");
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("slug", slug.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<PluginRow>>(0)?.into_iter().next())
}

/// The (possibly large) WASM module, Base64. Empty string = no module.
pub async fn get_wasm_b64(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<String>> {
    let mut res = db
        .query("SELECT VALUE wasm_b64 FROM plugin WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<String>>(0)?.into_iter().next())
}

pub async fn list_plugins(
    db: &Db,
    owner: &str,
    status: Option<&str>,
    q: Option<&str>,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<PluginRow>> {
    let mut clauses = vec!["owner = $owner".to_string()];
    if status.is_some() {
        clauses.push("status = $status".to_string());
    }
    if q.is_some() {
        clauses.push(
            "(string::lowercase(name) CONTAINS string::lowercase($q) \
             OR string::lowercase(description) CONTAINS string::lowercase($q))"
                .to_string(),
        );
    }
    let sql = format!(
        "{PLUGIN_SELECT} WHERE {} ORDER BY updated_at DESC LIMIT $limit START $offset",
        clauses.join(" AND ")
    );
    let mut query = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64));
    if let Some(s) = status {
        query = query.bind(("status", s.to_string()));
    }
    if let Some(s) = q {
        query = query.bind(("q", s.to_string()));
    }
    let mut res = query.await?.check()?;
    Ok(res.take(0)?)
}

/// All enabled plugins for an owner — what the registry loads at startup.
pub async fn enabled_plugins(db: &Db, owner: &str) -> AppResult<Vec<PluginRow>> {
    list_plugins(db, owner, Some("enabled"), None, 500, 0).await
}

/// Provenance of the write (threaded from the MCP `Actor` / REST handler).
pub struct Author<'a> {
    pub agent: Option<&'a str>,
    pub run_id: Option<&'a str>,
}

/// Create a plugin as a **forced draft**, or — with `replace` — re-author an
/// existing slug in place: bundle/manifest swapped, `version_int` bumped, and
/// status/review knocked BACK to draft/draft so every upgrade re-enters human
/// review (grant changes are re-reviewed). The caller must unload a replaced
/// plugin from the registry.
#[allow(clippy::too_many_arguments)]
pub async fn create_plugin(
    db: &Db,
    owner: &str,
    manifest: &Manifest,
    manifest_json: &str,
    kinds: &[String],
    wasm_b64: &str,
    wasm_bytes: &[u8],
    author: Author<'_>,
    replace: bool,
) -> AppResult<PluginRow> {
    let sha = bundle_sha256(manifest_json, wasm_bytes);

    let target_slug = slug::slugify(&manifest.name, "plugin");
    let existing = get_by_slug(db, owner, &target_slug).await?;
    // Replace is NAME-STABLE: the target is identified by the slug derived
    // from the manifest name. A replace whose slug matches nothing would
    // silently create a second plugin (orphaning the old one and its kv), so
    // refuse it instead of falling through to CREATE.
    if replace && existing.is_none() {
        return Err(AppError::BadRequest(format!(
            "replace=true but no plugin with slug `{target_slug}` exists — replace is \
             name-stable (the slug derives from the manifest name), so renaming via \
             replace is not supported; keep the name, or create a new plugin and \
             uninstall the old one"
        )));
    }
    if let Some(existing) = existing {
        if !replace {
            return Err(AppError::Conflict(format!(
                "plugin slug `{}` already exists; pass replace=true to upgrade it \
                 (the upgrade re-enters draft review)",
                existing.slug
            )));
        }
        let sql = format!(
            "UPDATE plugin SET name = $name, description = $description, version = $version, \
             version_int = version_int + 1, api_version = $api_version, manifest = $manifest, \
             kinds = $kinds, status = \"draft\", review = \"draft\", bundle_sha256 = $sha, \
             wasm_b64 = $wasm, author_agent = $agent, run_id = $run \
             WHERE owner = $owner AND uuid = $uuid; \
             {PLUGIN_SELECT} WHERE owner = $owner AND uuid = $uuid"
        );
        let mut res = db
            .query(sql)
            .bind(("owner", owner.to_string()))
            .bind(("uuid", existing.uuid))
            .bind(("name", manifest.name.clone()))
            .bind(("description", manifest.description.clone()))
            .bind(("version", manifest.version.clone()))
            .bind(("api_version", manifest.api_version as i64))
            .bind(("manifest", manifest_json.to_string()))
            .bind(("kinds", kinds.to_vec()))
            .bind(("sha", sha))
            .bind(("wasm", wasm_b64.to_string()))
            .bind(("agent", author.agent.map(str::to_string)))
            .bind(("run", author.run_id.map(str::to_string)))
            .await?
            .check()?;
        return res
            .take::<Vec<PluginRow>>(1)?
            .into_iter()
            .next()
            .ok_or_else(vanished);
    }

    let uuid = Uuid::new_v4().to_string();
    let plugin_slug = slug::unique_slug(db, owner, "plugin", &target_slug).await?;
    let sql = format!(
        "CREATE plugin CONTENT {{ uuid: $uuid, owner: $owner, name: $name, slug: $slug, \
         description: $description, version: $version, version_int: 1, \
         api_version: $api_version, manifest: $manifest, kinds: $kinds, \
         status: \"draft\", review: \"draft\", bundle_sha256: $sha, wasm_b64: $wasm, \
         author_agent: $agent, run_id: $run }}; \
         {PLUGIN_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("uuid", uuid))
        .bind(("owner", owner.to_string()))
        .bind(("name", manifest.name.clone()))
        .bind(("slug", plugin_slug))
        .bind(("description", manifest.description.clone()))
        .bind(("version", manifest.version.clone()))
        .bind(("api_version", manifest.api_version as i64))
        .bind(("manifest", manifest_json.to_string()))
        .bind(("kinds", kinds.to_vec()))
        .bind(("sha", sha))
        .bind(("wasm", wasm_b64.to_string()))
        .bind(("agent", author.agent.map(str::to_string)))
        .bind(("run", author.run_id.map(str::to_string)))
        .await?
        .check()?;
    res.take::<Vec<PluginRow>>(1)?
        .into_iter()
        .next()
        .ok_or_else(vanished)
}

/// The legal lifecycle transitions (current → requested). `review` follows
/// `status`: published on approve, back to draft on reject.
fn transition_allowed(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("draft", "approved")
            | ("draft", "rejected")
            | ("approved", "enabled")
            | ("approved", "rejected")
            | ("enabled", "disabled")
            | ("disabled", "enabled")
            | ("disabled", "rejected")
    )
}

/// Transition a plugin's lifecycle status (portal-only callers). Validates the
/// edge against the state machine and keeps `review` in lockstep (`published`
/// once approved, `draft` when rejected).
pub async fn set_status(db: &Db, owner: &str, uuid: &str, next: &str) -> AppResult<PluginRow> {
    if !PLUGIN_STATUSES.contains(&next) {
        return Err(AppError::BadRequest(format!(
            "invalid status `{next}` (expected one of: {})",
            PLUGIN_STATUSES.join(", ")
        )));
    }
    let Some(current) = get_plugin(db, owner, uuid).await? else {
        return Err(AppError::NotFound);
    };
    if !transition_allowed(&current.status, next) {
        return Err(AppError::Conflict(format!(
            "cannot transition plugin from `{}` to `{next}`",
            current.status
        )));
    }
    let review = match next {
        "approved" | "enabled" => "published",
        "rejected" => "draft",
        _ => return keep_review_transition(db, owner, uuid, next).await,
    };
    let sql = format!(
        "UPDATE plugin SET status = $status, review = $review \
         WHERE owner = $owner AND uuid = $uuid; \
         {PLUGIN_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .bind(("status", next.to_string()))
        .bind(("review", review.to_string()))
        .await?
        .check()?;
    res.take::<Vec<PluginRow>>(1)?
        .into_iter()
        .next()
        .ok_or_else(vanished)
}

/// Status-only update (disable keeps `review = "published"`).
async fn keep_review_transition(
    db: &Db,
    owner: &str,
    uuid: &str,
    next: &str,
) -> AppResult<PluginRow> {
    let sql = format!(
        "UPDATE plugin SET status = $status WHERE owner = $owner AND uuid = $uuid; \
         {PLUGIN_SELECT} WHERE owner = $owner AND uuid = $uuid"
    );
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .bind(("status", next.to_string()))
        .await?
        .check()?;
    res.take::<Vec<PluginRow>>(1)?
        .into_iter()
        .next()
        .ok_or_else(vanished)
}

/// Uninstall: scrub knowledge links, cascade the plugin's KV rows, delete the
/// row. The caller unloads it from the registry. Content the plugin authored
/// into core tables (ideas/pages/…) is owner-owned and survives by design.
pub async fn delete_plugin(db: &Db, owner: &str, uuid: &str) -> AppResult<Option<PluginRow>> {
    let Some(row) = get_plugin(db, owner, uuid).await? else {
        return Ok(None);
    };
    crate::knowledge::repo::scrub_item_links(db, owner, "plugin", uuid).await?;
    db.query("DELETE plugin_kv WHERE owner = $owner AND slug = $slug")
        .bind(("owner", owner.to_string()))
        .bind(("slug", row.slug.clone()))
        .await?
        .check()?;
    db.query("DELETE plugin WHERE owner = $owner AND uuid = $uuid")
        .bind(("owner", owner.to_string()))
        .bind(("uuid", uuid.to_string()))
        .await?
        .check()?;
    Ok(Some(row))
}

// ── plugin_kv: the only persistence surface plugin code gets ─────────────────

pub async fn kv_get(db: &Db, owner: &str, slug: &str, k: &str) -> AppResult<Option<String>> {
    let mut res = db
        .query("SELECT VALUE v FROM plugin_kv WHERE owner = $owner AND slug = $slug AND k = $k")
        .bind(("owner", owner.to_string()))
        .bind(("slug", slug.to_string()))
        .bind(("k", k.to_string()))
        .await?
        .check()?;
    Ok(res.take::<Vec<String>>(0)?.into_iter().next())
}

pub async fn kv_set(db: &Db, owner: &str, slug: &str, k: &str, v: &str) -> AppResult<()> {
    if k.is_empty() || k.len() > 200 {
        return Err(AppError::BadRequest(
            "kv key must be 1..=200 characters".into(),
        ));
    }
    if v.len() > MAX_KV_VALUE {
        return Err(AppError::PayloadTooLarge(format!(
            "kv value larger than {MAX_KV_VALUE} bytes"
        )));
    }
    // UPSERT via delete+create keeps the UNIQUE (owner, slug, k) index simple;
    // enforce the per-plugin key cap only on net-new keys.
    if kv_get(db, owner, slug, k).await?.is_none() {
        let mut res = db
            .query("SELECT VALUE count() FROM plugin_kv WHERE owner = $owner AND slug = $slug GROUP ALL")
            .bind(("owner", owner.to_string()))
            .bind(("slug", slug.to_string()))
            .await?
            .check()?;
        let count: Option<i64> = res.take::<Vec<i64>>(0)?.into_iter().next();
        if count.unwrap_or(0) >= MAX_KV_KEYS as i64 {
            return Err(AppError::BadRequest(format!(
                "plugin kv is full ({MAX_KV_KEYS} keys)"
            )));
        }
    }
    db.query(
        "DELETE plugin_kv WHERE owner = $owner AND slug = $slug AND k = $k; \
         CREATE plugin_kv CONTENT { owner: $owner, slug: $slug, k: $k, v: $v }",
    )
    .bind(("owner", owner.to_string()))
    .bind(("slug", slug.to_string()))
    .bind(("k", k.to_string()))
    .bind(("v", v.to_string()))
    .await?
    .check()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_edges() {
        // The happy path…
        assert!(transition_allowed("draft", "approved"));
        assert!(transition_allowed("approved", "enabled"));
        assert!(transition_allowed("enabled", "disabled"));
        assert!(transition_allowed("disabled", "enabled"));
        // …rejection from every non-terminal parked state…
        assert!(transition_allowed("draft", "rejected"));
        assert!(transition_allowed("approved", "rejected"));
        assert!(transition_allowed("disabled", "rejected"));
        // …and no shortcuts or resurrections.
        assert!(!transition_allowed("draft", "enabled"));
        assert!(!transition_allowed("enabled", "approved"));
        assert!(!transition_allowed("enabled", "rejected")); // disable first
        assert!(!transition_allowed("rejected", "approved"));
        assert!(!transition_allowed("rejected", "enabled"));
        assert!(!transition_allowed("draft", "draft"));
    }

    #[test]
    fn bundle_hash_is_stable_and_input_sensitive() {
        let a = bundle_sha256("{\"name\":\"x\"}", b"wasm");
        assert_eq!(a, bundle_sha256("{\"name\":\"x\"}", b"wasm"));
        assert_ne!(a, bundle_sha256("{\"name\":\"y\"}", b"wasm"));
        assert_ne!(a, bundle_sha256("{\"name\":\"x\"}", b"wasm2"));
        assert_eq!(a.len(), 64);
    }
}
