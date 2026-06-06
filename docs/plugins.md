# Baitler plugins

Plugins are **agent-authored, human-approved extensions**: an LLM (a Baitler
agent run, Claude Code over MCP, or any MCP client) writes a plugin, submits it
as a draft, the user approves + enables it in the portal, and the plugin's
tools become live capability for every future session. This is the platform's
self-improvement loop.

A plugin can ship three capability kinds (one plugin may mix them):

| Kind | Declared in | Executed as | Status |
|---|---|---|---|
| **MCP tool** | `tools[]` | a WASM export, advertised as `plugin__{slug}__{tool}` | ✅ Phase 16.B |
| **API endpoint** | `endpoints[]` | a WASM export behind `{METHOD} /px/{slug}/{path}` | ✅ Phase 16.C |
| **UI component** | `ui[]` | static assets in a sandboxed opaque-origin iframe | ✅ Phase 16.C |

## Enabling the system

Two switches, both off by default:

- `PLUGINS_ENABLED=true` in the backend env (`.env`) — activates the whole
  surface (MCP `plugins_*` tools, `/plugins` REST, registry loading).
- Build the backend with `--features plugins` — compiles the Extism/Wasmtime
  runtime (a heavy native dependency, hence the Cargo feature). Without it the
  full authoring/lifecycle surface still works; only **execution**
  (`plugins_test`, `plugins_invoke`, `plugin__*` calls) reports the runtime as
  unavailable.

## The lifecycle

```
 plugins_create (agent/MCP)         portal: Approve        portal: Enable
(nothing) ───────────────────► draft ───────────► approved ─────────► enabled
                                 │ review="draft"     review="published"  tools live
                                 │ status="draft"                   ┌──► disabled ─┐
                                 ▼                                  └──◄───────────┘
                            Review queue                 reject (terminal) / uninstall
```

- **Every write lands as a draft** — `plugins_create` ignores any
  `review`/`status` argument. Drafts appear in the portal review queue
  (`review_list` / `GET /review`) beside agent-drafted ideas and documents.
- **Approve / enable / disable / reject are portal-only REST verbs**
  (`POST /plugins/{id}/approve|enable|disable|reject`). There is deliberately
  **no MCP verb** for any lifecycle transition: an agent can author and
  propose, never self-approve. `plugins_uninstall` exists over MCP for host
  clients but is refused for Baitler-spawned runs.
- **Upgrades re-enter review**: `plugins_create {replace: true}` swaps the
  bundle on the same slug, bumps `version_int`, knocks status/review back to
  draft, and unloads the old version. Replace is **name-stable** (the slug
  derives from the manifest name; a replace naming a non-existent slug is
  refused rather than silently creating a second plugin), and a
  **Baitler-spawned run cannot replace an approved/enabled plugin** — it may
  re-author its own un-reviewed drafts, but overwriting a human-vetted
  artifact (and unloading the live capability) is portal territory.
- **New tools appear on the next MCP session.** The transport is the
  JSON-response Streamable HTTP variant (no server→client stream), so a client
  sees newly enabled plugin tools when it next lists tools (e.g. the next
  agent run). Enable is human-gated and inherently async, so this is fine in
  practice; `plugins_invoke {slug, tool, input}` reaches any enabled tool
  immediately without a re-list.

## Authoring (the agent loop)

1. **`plugins_scaffold {name, kind}`** → a ready-to-edit manifest + an Extism
   JS guest skeleton + build steps.
2. Write the guest (JavaScript via the [Extism JS PDK](https://github.com/extism/js-pdk);
   Rust/Go/Python PDKs work too) and compile to one `plugin.wasm`. Install the
   compiler via the PDK's release script (needs Binaryen on `PATH`):
   `curl -O https://raw.githubusercontent.com/extism/js-pdk/main/install.sh && bash install.sh`,
   then `extism-js plugin.js -o plugin.wasm`.
3. **`plugins_validate {manifest, wasm_b64}`** → structured
   `{path, msg, hint}` diagnostics. Fix, repeat.
4. **`plugins_test {manifest, wasm_b64, export, input}`** → runs the export in
   a **throwaway sandbox**: same runtime and grants, but `plugin_kv` is
   in-memory and content writes are refused, so nothing persists. Failures
   return as data (`{ok:false, error}`), not tool errors.
5. **`plugins_create {manifest, wasm_b64}`** → the forced draft. Tell the user
   it is ready for review.
6. The user approves + enables in the portal. Done — `plugin__{slug}__{tool}`
   is in `tools/list` for every future session.

`plugins_list` / `plugins_get` inspect installed plugins;
`plugins_export {slug}` returns the full bundle (manifest + `wasm_b64` +
`sha256`) for backup or transfer.

## The manifest

```jsonc
{
  "name": "CSV Stats",                  // slug derived: csv-stats
  "version": "0.1.0",
  "api_version": 1,                     // host plugin-ABI epoch (see below)
  "description": "Summarize CSV data",  // indexed for knowledge_search
  "tools": [{
    "name": "summarize",                // [a-z0-9_], no "__" run
    "export": "summarize",              // the WASM export invoked
    "description": "Summarize a CSV",
    "input_schema": { "type": "object", "properties": { /* … */ }, "required": [] }
  }],
  "endpoints": [],                      // Phase 16.C
  "ui": [],                             // Phase 16.C
  "capabilities": {                     // DEFAULT-DENY — absent = denied
    "host_fns": ["log", "plugin_kv_get", "plugin_kv_set"],
    "egress": [],                       // must be empty in v1 (unwired)
    "secrets": [],                      // must be empty in v1 (unwired)
    "memory_max_pages": 256,            // 64KiB pages; ceiling 1024 (64 MiB)
    "timeout_ms": 2000                  // wall-clock kill; ceiling 30000
  }
}
```

Unknown fields anywhere are **rejected at parse** (there is no filesystem
capability and no way to declare one — a plugin can never touch the disk, so
even a restricted agent run that authored a plugin gains nothing it didn't
already have).

## Host functions (the closed enum)

Plugin code reaches Baitler **only** through these owner-scoped host
functions, each a thin wrapper over an existing repo. Input and output are
JSON strings. Grants are structural: only granted functions are registered,
so a guest importing anything else fails at instantiation.

| Host fn | Effect |
|---|---|
| `log` | `{…}` → tracing log line tagged with the plugin slug |
| `kn_search` | `{q, limit?}` → `knowledge_search` results |
| `ideas_get` | `{id}` → the idea, or `null` |
| `ideas_create` | `{title, body?, tags?}` → creates an idea — **forced `review:"draft"`** |
| `pages_create` | `{title, body?, source_format?}` → creates a page — **forced `visibility:"draft"`** |
| `plugin_kv_get` | `{k}` → `{v}` from the plugin's own KV store |
| `plugin_kv_set` | `{k, v}` → upsert (≤64 KB/value, ≤1000 keys) |

`plugin_kv` is the **only** persistence plugin code gets (plugins never define
tables); it is keyed on `(owner, slug)` so state survives upgrades and is
cascaded away on uninstall. Content written through host fns records its own
`activity` rows attributed to `plugin:{slug}` plus the invoking run's
`run_id` — so the butler report shows exactly what a plugin did.

## API endpoints (the `endpoints[]` kind)

Enabled plugins serve HTTP under **`/px/{slug}/…`** (a deliberate prefix apart
from the `/plugins` management surface, so a plugin path like `approve` can
never collide with a lifecycle verb). One static catch-all dispatches every
endpoint — plugins never register routes. The bound export receives
`{method, path, query, body, owner}` as JSON and may return either a plain
JSON value (served as `application/json`) or `{status?, content_type?, body}`
— content types are whitelisted (`application/json`, `text/plain`, `text/csv`,
`text/html`, `image/svg+xml`; HTML/SVG are sanitized server-side) and status
is limited to 200–499. The whole dispatch sits under a route-layer deadline
(`timeout_ms` + 5 s grace → 504) on top of the in-sandbox epoch kill, and
every call logs a `plugin.invoke` activity row.

## UI components (the `ui[]` kind)

Declare mounts in `ui[]` (`slot`: `detail` | `rail` | `butler_widget` |
`object`) and upload the assets with `plugins_create {ui_assets: {"<path>":
"<base64>", …}}` (≤32 files, ≤2 MiB decoded; every declared `entry` must be
present; covered by the bundle digest). While enabled, assets serve from
**`GET /plugin-ui/{uuid}/{path}`** — outside the CORS layer, on an **opaque,
cookieless origin**: `sandbox allow-scripts` without `allow-same-origin`,
`connect-src 'none'`, content types from a closed extension list with
`nosniff`, and `frame-ancestors` pinned to the app origins. Drafts/disabled
plugins 404.

The portal mounts each slot in a sandboxed `<iframe>` via `PluginFrame`
(`frontend/src/features/plugins/`). Since the frame cannot fetch
(`connect-src 'none'`) and holds no credentials, its ONLY path to Baitler is
the **postMessage bridge**, which the host pins to the plugin's own
`/px/{slug}/…` endpoints and relays through the credentialed client:

```js
// inside plugin UI code
window.parent.postMessage(
  { baitler: 'fetch', id: 'r1', path: '/px/csv-stats/summarize', method: 'POST', body: {…} },
  '*',
);
window.addEventListener('message', (e) => {
  if (e.data?.baitler === 'result' && e.data.id === 'r1') {
    // e.data.ok, e.data.body | e.data.error
  }
});
// the host also posts { baitler: 'init', slug, theme } on load
```

Manage everything (review queue with the capability diff, approve/reject,
enable/disable, uninstall, and each plugin's detail surface) on the portal's
**Plugins** page (`/plugins`).

## Containment summary

- **Sandbox**: Extism on Wasmtime, in-process; a **fresh instance per
  invocation**; no WASI, no filesystem, no network; `memory_max_pages` +
  `timeout_ms` (epoch interruption) from the validated manifest.
- **Provenance**: the bundle is pinned by `bundle_sha256` at create and
  re-verified on every registry load — a tampered artifact refuses to run
  (and the row auto-rolls to `disabled`). Every invocation logs a
  `plugin.invoke` activity row.
- **ABI epochs**: `api_version` is checked at load; after a host ABI bump,
  stale plugins are auto-disabled (loudly) instead of breaking silently.
- **Run restrictions compose**: a Baitler-spawned agent run may scaffold,
  validate, test, create (draft), and invoke *enabled* plugins — it cannot
  uninstall, cannot reach any approve/enable path (none exists over MCP), and
  no plugin can return disk access the run doesn't have (no such host fn).

## Testing

`backend/crates/api/tests/plugins.rs` covers the lifecycle without the
runtime; `tests/plugins_runtime.rs` (under `--features plugins`) executes
hand-written WAT fixtures (`tests/fixtures/*.wat`) covering echo dispatch, the
timeout kill, host-fn grants/refusals, KV persistence + uninstall cascade,
throwaway test sandboxes, and digest-mismatch refusal. CI runs both
configurations.
