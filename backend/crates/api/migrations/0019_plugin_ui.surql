-- 0019_plugin_ui — UI assets for the Phase 16.C UI plugin kind.
--
-- `ui_assets` is a JSON object body mapping bundle-relative asset paths to
-- Base64 contents (e.g. {"ui/index.html": "...", "ui/app.js": "..."}),
-- validated and capped on write (≤32 files, ≤2 MiB decoded total, path-safe
-- keys) and covered by `bundle_sha256` alongside the manifest and WASM.
-- Served ONLY via the outside-CORS opaque-origin route GET /plugin-ui/{uuid}/…
-- and only while status = "enabled". Excluded from the standard projection
-- (like `wasm_b64` — it can be large).

DEFINE FIELD IF NOT EXISTS ui_assets ON plugin TYPE string DEFAULT "";
