//! The plugin manifest: what a plugin declares, validated before any write.
//!
//! Deny-unknown-fields throughout, so a manifest can never smuggle a capability
//! the host doesn't know about (e.g. there is NO filesystem field — rejecting
//! the surface beats trusting a reviewer to never approve it). Capabilities are
//! **default-deny**: absent means denied, and validation re-checks every grant
//! against the host ceiling. Issues come back as structured `{path, msg, hint}`
//! entries so an authoring agent can iterate fast (`plugins_validate`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Host plugin-ABI epoch. Bumped when the host-fn surface changes shape; the
/// registry refuses (auto-disables) plugins built against a different epoch
/// rather than letting a platform migration silently break them.
pub const HOST_API_VERSION: u32 = 1;

/// The closed enum of host functions a plugin may request. Each is a thin
/// owner-scoped wrapper over an existing repo (`hostfns.rs`) — there is no
/// filesystem function and no way to declare one.
pub const HOST_FNS: &[&str] = &[
    "log",
    "kn_search",
    "ideas_get",
    "ideas_create",
    "pages_create",
    "plugin_kv_get",
    "plugin_kv_set",
];

/// Plugin capability kinds, derived from which manifest arrays are non-empty.
pub const KINDS: &[&str] = &["mcp_tool", "api_endpoint", "ui_component"];

/// UI mount slots (served/rendered in Phase 16.C).
pub const UI_SLOTS: &[&str] = &["object", "rail", "butler_widget", "detail"];

const MAX_NAME: usize = 100;
const MAX_VERSION: usize = 50;
const MAX_DESCRIPTION: usize = 1_000;
const MAX_TOOLS: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_UI: usize = 8;
const MAX_TOOL_DESCRIPTION: usize = 500;
/// 64 MiB of guest memory, absolute ceiling (pages are 64 KiB).
const MAX_MEMORY_PAGES: u32 = 1_024;
const MAX_TIMEOUT_MS: u64 = 30_000;

fn default_memory_pages() -> u32 {
    256 // 16 MiB
}

fn default_timeout_ms() -> u64 {
    2_000
}

/// A validated plugin manifest (the `manifest` JSON body of a `plugin` row).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_api_version")]
    pub api_version: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tools: Vec<ToolDecl>,
    #[serde(default)]
    pub endpoints: Vec<EndpointDecl>,
    #[serde(default)]
    pub ui: Vec<UiDecl>,
    #[serde(default)]
    pub capabilities: Capabilities,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_api_version() -> u32 {
    HOST_API_VERSION
}

/// One MCP tool the plugin contributes (advertised as `plugin__{slug}__{name}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDecl {
    pub name: String,
    /// WASM export invoked for this tool.
    pub export: String,
    #[serde(default)]
    pub description: String,
    /// JSON Schema for the tool arguments (object schema, MCP wire shape).
    #[serde(default = "default_schema")]
    pub input_schema: Value,
}

fn default_schema() -> Value {
    serde_json::json!({ "type": "object", "properties": {}, "required": [] })
}

/// One HTTP endpoint the plugin serves under `/plugins/{slug}/{path}` (16.C).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointDecl {
    pub method: String,
    /// Path under the plugin mount, no leading slash, no `..` segments.
    pub path: String,
    pub export: String,
}

/// One UI component the plugin mounts (served opaque-origin in 16.C).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiDecl {
    pub slot: String,
    pub key: String,
    #[serde(default)]
    pub label: String,
    /// Bundle-relative entry asset (e.g. `ui/index.html`).
    pub entry: String,
}

/// Default-deny capability grants. Every field empty/minimal unless declared;
/// validation intersects each grant with the host ceiling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// Host functions the plugin may import (⊆ [`HOST_FNS`]).
    #[serde(default)]
    pub host_fns: Vec<String>,
    /// Outbound-HTTP allowlist. **Always empty in v1** — the field exists so a
    /// future grant is a review-visible manifest change, but it is unwired and
    /// validation rejects any entry.
    #[serde(default)]
    pub egress: Vec<String>,
    /// Names of per-owner secrets (Phase 6 encrypted store) injected as config
    /// vars. Unwired in v1; validation rejects any entry.
    #[serde(default)]
    pub secrets: Vec<String>,
    #[serde(default = "default_memory_pages")]
    pub memory_max_pages: u32,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            host_fns: Vec::new(),
            egress: Vec::new(),
            secrets: Vec::new(),
            memory_max_pages: default_memory_pages(),
            timeout_ms: default_timeout_ms(),
        }
    }
}

/// One structured validation problem (`plugins_validate` returns a list).
#[derive(Debug, Serialize)]
pub struct Issue {
    /// JSON-path-ish locator, e.g. `tools[0].name`.
    pub path: String,
    pub msg: String,
    pub hint: String,
}

impl Issue {
    fn new(path: &str, msg: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            path: path.to_string(),
            msg: msg.into(),
            hint: hint.into(),
        }
    }
}

/// Parse manifest JSON. A shape/unknown-field error comes back as one issue
/// rather than an `Err`, so `plugins_validate` reports it structurally.
pub fn parse(raw: &Value) -> Result<Manifest, Issue> {
    serde_json::from_value(raw.clone()).map_err(|e| {
        Issue::new(
            "manifest",
            format!("manifest does not match the schema: {e}"),
            "fields are deny-unknown; start from plugins_scaffold output",
        )
    })
}

/// Validate a parsed manifest against the host ceiling. Empty result = valid.
pub fn validate(m: &Manifest, has_wasm: bool) -> Vec<Issue> {
    let mut issues = Vec::new();

    if m.name.trim().is_empty() || m.name.len() > MAX_NAME {
        issues.push(Issue::new(
            "name",
            format!("name must be 1..={MAX_NAME} characters"),
            "a short human-readable plugin name; the slug is derived from it",
        ));
    }
    if m.version.trim().is_empty() || m.version.len() > MAX_VERSION {
        issues.push(Issue::new(
            "version",
            format!("version must be 1..={MAX_VERSION} characters"),
            "use a semver-ish string like 0.1.0",
        ));
    }
    if m.description.len() > MAX_DESCRIPTION {
        issues.push(Issue::new(
            "description",
            format!("description longer than {MAX_DESCRIPTION} characters"),
            "keep it to a short paragraph; it is indexed for knowledge_search",
        ));
    }
    if m.api_version != HOST_API_VERSION {
        issues.push(Issue::new(
            "api_version",
            format!(
                "api_version {} does not match the host plugin ABI ({HOST_API_VERSION})",
                m.api_version
            ),
            "rebuild the plugin against the current host API",
        ));
    }
    if m.tools.is_empty() && m.endpoints.is_empty() && m.ui.is_empty() {
        issues.push(Issue::new(
            "manifest",
            "plugin declares no capabilities",
            "declare at least one of tools[], endpoints[], ui[]",
        ));
    }
    if (!m.tools.is_empty() || !m.endpoints.is_empty()) && !has_wasm {
        issues.push(Issue::new(
            "manifest",
            "tools/endpoints declared but no WASM module supplied",
            "pass wasm_b64 (an Extism guest module) alongside the manifest",
        ));
    }

    validate_tools(m, &mut issues);
    validate_endpoints(m, &mut issues);
    validate_ui(m, &mut issues);
    validate_capabilities(&m.capabilities, &mut issues);

    issues
}

/// The capability kinds this manifest ships (stored on the row for filtering).
pub fn kinds(m: &Manifest) -> Vec<String> {
    let mut kinds = Vec::new();
    if !m.tools.is_empty() {
        kinds.push("mcp_tool".to_string());
    }
    if !m.endpoints.is_empty() {
        kinds.push("api_endpoint".to_string());
    }
    if !m.ui.is_empty() {
        kinds.push("ui_component".to_string());
    }
    kinds
}

/// Valid tool-name charset: lowercase alnum + underscore (MCP-name safe; the
/// advertised name is `plugin__{slug}__{tool}` so `__` stays an unambiguous
/// separator — single underscores inside the tool name are fine).
fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.contains("__")
}

fn validate_tools(m: &Manifest, issues: &mut Vec<Issue>) {
    if m.tools.len() > MAX_TOOLS {
        issues.push(Issue::new(
            "tools",
            format!("at most {MAX_TOOLS} tools per plugin"),
            "split into several plugins",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for (i, t) in m.tools.iter().enumerate() {
        let path = format!("tools[{i}]");
        if !valid_tool_name(&t.name) {
            issues.push(Issue::new(
                &format!("{path}.name"),
                "tool names are 1..=64 of [a-z0-9_], with no `__` run",
                "`__` is the namespace separator in plugin__{slug}__{tool}",
            ));
        }
        if !seen.insert(t.name.clone()) {
            issues.push(Issue::new(
                &format!("{path}.name"),
                format!("duplicate tool name `{}`", t.name),
                "tool names must be unique within a plugin",
            ));
        }
        if t.export.trim().is_empty() {
            issues.push(Issue::new(
                &format!("{path}.export"),
                "export must name a WASM export",
                "the Extism guest function invoked for this tool",
            ));
        }
        if t.description.len() > MAX_TOOL_DESCRIPTION {
            issues.push(Issue::new(
                &format!("{path}.description"),
                format!("description longer than {MAX_TOOL_DESCRIPTION} characters"),
                "one or two sentences",
            ));
        }
        let schema_ok = t.input_schema.get("type").and_then(Value::as_str) == Some("object")
            && t.input_schema
                .get("properties")
                .is_some_and(Value::is_object);
        if !schema_ok {
            issues.push(Issue::new(
                &format!("{path}.input_schema"),
                "input_schema must be a JSON Schema object: { type: \"object\", properties: {…} }",
                "mirror the shape of the built-in tool schemas (tools/list)",
            ));
        }
    }
}

fn validate_endpoints(m: &Manifest, issues: &mut Vec<Issue>) {
    if m.endpoints.len() > MAX_ENDPOINTS {
        issues.push(Issue::new(
            "endpoints",
            format!("at most {MAX_ENDPOINTS} endpoints per plugin"),
            "split into several plugins",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for (i, e) in m.endpoints.iter().enumerate() {
        let path = format!("endpoints[{i}]");
        if !matches!(
            e.method.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
        ) {
            issues.push(Issue::new(
                &format!("{path}.method"),
                format!("invalid method `{}`", e.method),
                "one of GET, POST, PUT, PATCH, DELETE (uppercase)",
            ));
        }
        let p = &e.path;
        let path_ok = !p.is_empty()
            && p.len() <= 200
            && !p.starts_with('/')
            && p.split('/').all(|seg| {
                !seg.is_empty()
                    && seg != ".."
                    && seg != "."
                    && seg
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            });
        if !path_ok {
            issues.push(Issue::new(
                &format!("{path}.path"),
                "path must be relative segments of [A-Za-z0-9._-], no leading slash, no `..`",
                "e.g. `summarize` or `reports/daily`",
            ));
        }
        if !seen.insert((e.method.clone(), e.path.clone())) {
            issues.push(Issue::new(
                &format!("{path}.path"),
                format!("duplicate endpoint `{} {}`", e.method, e.path),
                "method+path must be unique within a plugin",
            ));
        }
        if e.export.trim().is_empty() {
            issues.push(Issue::new(
                &format!("{path}.export"),
                "export must name a WASM export",
                "the Extism guest function invoked for this endpoint",
            ));
        }
    }
}

/// Bundle-relative asset path rule, shared by manifest validation, the
/// `plugins_create` upload cap, and the public serve boundary: relative
/// segments of `[A-Za-z0-9._-]`, no `..`/`.`, no leading slash.
pub fn valid_asset_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && path.split('/').all(|seg| {
            !seg.is_empty()
                && seg != ".."
                && seg != "."
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
}

fn validate_ui(m: &Manifest, issues: &mut Vec<Issue>) {
    if m.ui.len() > MAX_UI {
        issues.push(Issue::new(
            "ui",
            format!("at most {MAX_UI} UI components per plugin"),
            "split into several plugins",
        ));
    }
    for (i, u) in m.ui.iter().enumerate() {
        let path = format!("ui[{i}]");
        if !UI_SLOTS.contains(&u.slot.as_str()) {
            issues.push(Issue::new(
                &format!("{path}.slot"),
                format!("invalid slot `{}`", u.slot),
                format!("one of: {}", UI_SLOTS.join(", ")),
            ));
        }
        if u.key.trim().is_empty() || u.key.len() > 64 {
            issues.push(Issue::new(
                &format!("{path}.key"),
                "key must be 1..=64 characters",
                "a stable identifier for this mount",
            ));
        }
        if !valid_asset_path(&u.entry) {
            issues.push(Issue::new(
                &format!("{path}.entry"),
                "entry must be a bundle-relative asset path of [A-Za-z0-9._-] segments \
                 (no `..`, no leading slash)",
                "e.g. `ui/index.html`",
            ));
        }
    }
}

fn validate_capabilities(c: &Capabilities, issues: &mut Vec<Issue>) {
    for (i, f) in c.host_fns.iter().enumerate() {
        if !HOST_FNS.contains(&f.as_str()) {
            issues.push(Issue::new(
                &format!("capabilities.host_fns[{i}]"),
                format!("unknown host function `{f}`"),
                format!("the closed set is: {}", HOST_FNS.join(", ")),
            ));
        }
    }
    if !c.egress.is_empty() {
        issues.push(Issue::new(
            "capabilities.egress",
            "network egress is not available in v1",
            "leave the allowlist empty; egress lands in a later phase",
        ));
    }
    if !c.secrets.is_empty() {
        issues.push(Issue::new(
            "capabilities.secrets",
            "secret injection is not available in v1",
            "leave secrets empty; injection lands in a later phase",
        ));
    }
    if c.memory_max_pages == 0 || c.memory_max_pages > MAX_MEMORY_PAGES {
        issues.push(Issue::new(
            "capabilities.memory_max_pages",
            format!("memory_max_pages must be 1..={MAX_MEMORY_PAGES} (64KiB pages)"),
            "the default is 256 (16 MiB)",
        ));
    }
    if c.timeout_ms == 0 || c.timeout_ms > MAX_TIMEOUT_MS {
        issues.push(Issue::new(
            "capabilities.timeout_ms",
            format!("timeout_ms must be 1..={MAX_TIMEOUT_MS}"),
            "the default is 2000",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_tool_manifest() -> Value {
        json!({
            "name": "CSV Stats",
            "tools": [
                { "name": "summarize", "export": "summarize", "description": "d" }
            ],
            "capabilities": { "host_fns": ["log"] }
        })
    }

    #[test]
    fn minimal_manifest_parses_and_validates() {
        let m = parse(&minimal_tool_manifest()).expect("parse");
        assert_eq!(m.api_version, HOST_API_VERSION);
        assert_eq!(m.capabilities.memory_max_pages, 256);
        assert!(validate(&m, true).is_empty());
        assert_eq!(kinds(&m), vec!["mcp_tool"]);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let raw = json!({ "name": "x", "tools": [], "fs": ["/"] });
        assert!(parse(&raw).is_err());
        let raw = json!({ "name": "x", "capabilities": { "allowed_paths": ["/"] } });
        assert!(parse(&raw).is_err());
    }

    #[test]
    fn tools_without_wasm_is_invalid() {
        let m = parse(&minimal_tool_manifest()).unwrap();
        let issues = validate(&m, false);
        assert!(issues.iter().any(|i| i.msg.contains("no WASM module")));
    }

    #[test]
    fn egress_secrets_and_unknown_host_fns_are_rejected() {
        let raw = json!({
            "name": "x",
            "tools": [{ "name": "t", "export": "t" }],
            "capabilities": {
                "host_fns": ["workspace_read"],
                "egress": ["api.example.com"],
                "secrets": ["openai"]
            }
        });
        let m = parse(&raw).unwrap();
        let issues = validate(&m, true);
        assert!(issues.iter().any(|i| i.path == "capabilities.host_fns[0]"));
        assert!(issues.iter().any(|i| i.path == "capabilities.egress"));
        assert!(issues.iter().any(|i| i.path == "capabilities.secrets"));
    }

    #[test]
    fn tool_name_rules() {
        assert!(valid_tool_name("summarize_csv2"));
        assert!(!valid_tool_name("Summarize"));
        assert!(!valid_tool_name("a__b"));
        assert!(!valid_tool_name(""));
    }

    #[test]
    fn endpoint_path_traversal_is_rejected() {
        let raw = json!({
            "name": "x",
            "endpoints": [{ "method": "POST", "path": "../etc", "export": "h" }]
        });
        let m = parse(&raw).unwrap();
        let issues = validate(&m, true);
        assert!(issues.iter().any(|i| i.path == "endpoints[0].path"));
    }

    #[test]
    fn api_version_mismatch_is_flagged() {
        let raw = json!({
            "name": "x",
            "api_version": 99,
            "tools": [{ "name": "t", "export": "t" }]
        });
        let m = parse(&raw).unwrap();
        assert!(validate(&m, true).iter().any(|i| i.path == "api_version"));
    }
}
