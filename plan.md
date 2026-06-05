# Baitler — Build Plan

> Personal-assistant & data-organizer platform ("Butler" + "AI"). Source of truth for
> scope is `Vibecoding/instructions.md`; this plan turns that brief into an ordered,
> step-by-step build. Check off steps as they land. Update `CLAUDE.md`'s **Commands**
> section whenever a new runnable script appears.

## Stack at a glance

| Surface   | Tech |
|-----------|------|
| Frontend  | React + Vite + TypeScript + TailwindCSS |
| Backend   | Rust HTTP API (Axum) + SurrealDB |
| Mobile    | Native iOS (Swift) + Android (Kotlin) |
| Tooling   | `scripts/` for combined dev-run and builds |

## Guiding principles (from the brief)

- **LLM access is multi-provider, multi-model, multi-modal.** Build a provider
  abstraction before writing any vendor-specific code.
- **Document conversion is a shared pathway.** HTML ↔ Markdown ↔ PDF ↔ MS Office
  flows through one service, not per-feature one-offs.
- **Auth is OAuth2-first.** No username/password; identity comes from Google, GitHub, etc.
- **Monorepo.** Separate workspaces (`frontend/`, `backend/`, `mobile/`) sharing
  top-level `scripts/`.

---

## Phase 0 — Repository & tooling foundation

Goal: an empty-but-runnable monorepo skeleton with version control, formatting, and CI.

- [x] **0.1** `git init`; add `.gitignore` (Rust `target/`, Node `node_modules/`, `dist/`, `.env`, SurrealDB data dirs).
- [x] **0.2** Create monorepo layout: `frontend/`, `backend/`, `mobile/ios/`, `mobile/android/`, `scripts/`, `docs/`.
- [x] **0.3** Add root `README.md` (quickstart) and `.env.example` documenting every required env var.
- [x] **0.4** Decide & document toolchain versions: `.nvmrc` (Node), `rust-toolchain.toml` (Rust).
- [x] **0.5** Add editor/format config: `.editorconfig`, `.prettierrc.json`, `rustfmt.toml` (+ Clippy via toolchain). ESLint ships with the frontend scaffold (Phase 3, needs installed React/TS plugins).
- [x] **0.6** `scripts/dev.sh` — run frontend + backend + SurrealDB together; degrades gracefully until services are scaffolded.
- [x] **0.7** `scripts/build.sh` — build frontend (`vite build`) and backend (`cargo build --release`).
- [x] **0.8** CI skeleton (GitHub Actions): lint + test for frontend and backend on PR.
- [x] **0.9** Update `CLAUDE.md` **Commands** section with the real commands now that they exist.

## Phase 1 — Backend skeleton & SurrealDB

Goal: a Rust API that boots, connects to SurrealDB, and answers a health check.

- [x] **1.1** Cargo **workspace** at `backend/` with member `crates/api` (binary `baitler-api`, **Axum** + Tokio); `default-members` so `cargo run` works.
- [x] **1.2** Core deps: `axum` 0.8, `tokio`, `tower-http` (cors/trace), `serde`, `surrealdb` 2 (`any` engine: `kv-mem`/`protocol-ws`/`protocol-http`), `tracing`(+subscriber), `thiserror`, `dotenvy`, `include_dir`.
- [x] **1.3** `Config::from_env` (`PORT`, `BIND_HOST`, `CORS_ALLOWED_ORIGINS`, `SURREAL_*`, `SURREAL_TIMEOUT_SECS`) with **fail-fast validation** (wildcard/invalid origins, partial creds) and password redaction.
- [x] **1.4** SurrealDB connect via the `any` engine (`memory`/`ws`/`http`); embedded migration runner applies `migrations/*.surql` **atomically** (apply + bookkeeping in one transaction), idempotently.
- [x] **1.5** `GET /health` (timeout-bounded DB ping → 200/503) and `GET /version`; `tracing-subscriber` (EnvFilter, optional JSON) + `tower-http` `TraceLayer`.
- [x] **1.6** `AppError` (thiserror) → JSON `{error:{code,message}}` envelope with server-fault sanitization; `CorsLayer` from validated origins with credentials.
- [x] **1.7** Integration harness boots the app on an ephemeral port against an embedded `memory` DB. **18 tests** (health/version/404/CORS×2/migration-idempotency + config/error/db units), all green.

> **Zero-install dev:** `SURREAL_URL=memory` runs SurrealDB in-process, so the API and
> its tests need no `surreal` server or Docker. Set `ws://…` for a real datastore.
>
> Verified green: `cargo build`, `cargo clippy -D warnings`, `cargo test` (18), `cargo fmt --check`,
> plus a runtime smoke test. Hardened via an adversarial multi-lens review (42 findings → 15 applied);
> deferred items: `_migration` UNIQUE index (multi-replica), graceful-shutdown deadline, request-id
> spans, global `TimeoutLayer` — revisit in later phases.

## Phase 2 — Auth (OAuth2-first)

Goal: users sign in via Google/GitHub; sessions are issued and verified.

- [ ] **2.1** Define `User`, `Identity` (provider + provider_user_id), and `Session` tables in SurrealDB.
- [ ] **2.2** OAuth2 authorization-code flow for Google and GitHub (`oauth2` crate); pluggable provider trait so new providers drop in.
- [ ] **2.3** Callback handler: upsert user + identity, issue session (JWT or opaque token + secure cookie).
- [ ] **2.4** Auth middleware/extractor; `GET /me` returns the current user.
- [ ] **2.5** Logout + token refresh/expiry handling.
- [ ] **2.6** Frontend: login screen, OAuth redirect handling, session persistence, auth context/guarded routes.
- [ ] **2.7** Tests for the full login/callback/session lifecycle.

## Phase 3 — Frontend shell & base portal

Goal: the central hub UI that every feature plugs into.

- [x] **3.1** Scaffolded `frontend` with Vite 8 + React 19 + TS (strict); TailwindCSS v4 via `@tailwindcss/vite`; `@`→`src` alias; Vite `envDir`→repo root so one `.env` serves all workspaces.
- [x] **3.2** App shell: React Router 7 routes; `AppLayout` (responsive sidebar + sticky header, mobile drawer with focus trap + scroll lock); light/dark theme via Zustand + CSS-variable design tokens (amber "honey" accent); pre-paint inline script avoids FOUC.
- [x] **3.3** Typed `apiFetch` client (base URL from `VITE_API_BASE_URL`, `credentials:'include'` for future cookies, `AbortSignal` passthrough, error-envelope → `ApiError`, prod fail-loud); TanStack Query 5 + `useHealth`/`useVersion`.
- [x] **3.4** **Base portal** dashboard: feature cards linking to files/ideas/documents/analytics/AI (phase-badged); live **System status** panel (API/DB/version); placeholder pages per feature route + 404.
- [x] **3.5** UI kit (`Button`/`Card`/`Badge`/`Spinner` with `cn()` = clsx+tailwind-merge) + Zustand theme store; `ErrorBoundary`; skip-link; auth-ready header `UserMenu` stub.
- [ ] **3.6** ~~Wire the Phase 2 auth flow into the shell; protected layout.~~ **Deferred** — auth is being added in the final phase (per direction). Shell is auth-*ready*: credentialed client + UserMenu slot.

> Verified green: `tsc -b` (strict + `noUncheckedIndexedAccess`), `eslint .`, `vitest run`
> (**24 tests**: api/routing/theme+apply/status/Button/Dashboard), `vite build`, plus a
> live full-stack run (headless screenshot showed the base portal with "API connected" +
> version from the real backend). Hardened via adversarial multi-lens review (54 findings →
> 42 verified → ~20 applied, incl. two WCAG fixes: feature-card focus ring, amber-text contrast).
> Deferred: theme `system` mode, per-route `<title>`, CSP/security headers, env typing — later phases.
>
> **Zero-install dev:** `SURREAL_URL=memory` (backend) means `./scripts/dev.sh` runs the whole stack with no `surreal`/Docker.

## Phase 4 — File storage & management

Goal: upload, browse, organize, download files.

- [x] **4.1** `Storage` trait (dyn, async) + `LocalStorage` (objects keyed by server UUID → no path traversal; temp-file+rename writes). S3 deferred behind the same trait.
- [x] **4.2** `file`/`folder` tables (migration `0002`): owner, name, mime, size, `folder_id`/`parent_id` hierarchy, `storage_key`, timestamps; unique + owner indexes. Public ids are UUIDs (internal record ids never exposed).
- [x] **4.3** Multipart upload (size-limited via `DefaultBodyLimit`→413, MIME validated, batch-atomic rollback), **streamed** download (allowlisted inline + `nosniff` + CSP, RFC 6266 `filename*`), rename/move (PATCH double-option), delete, folder create/rename/move(cycle-guarded)/delete-empty.
- [x] **4.4** Listing with breadcrumbs + name search (case-insensitive, cross-folder) + pagination; all owner-scoped.
- [x] **4.5** Frontend file manager: breadcrumb nav, folder/file grid, drag-and-drop + button upload, new folder, rename, delete (accessible ConfirmModal), download + image preview (credentialed blob), live region + states.
- [x] **4.6** Tests: **30 backend** (lifecycle, move-into-folder, cycle guards, 413/400/404, search pagination/case, breadcrumbs, secure download headers, repo-level **owner isolation** with two synthetic owners) + **26 frontend**.

> **Ownership while auth is deferred:** every query is owner-scoped, fed by a `CurrentOwner`
> stub returning a fixed dev owner — the auth phase swaps only that extractor. Isolation is
> proven at the repo layer with two owners. The content endpoint is the one path that needs
> rework under cookie auth (uses a credentialed blob fetch on the client now to stay ready).
>
> Verified green: backend `build`/`clippy -D warnings`/`test` (30)/`fmt` + live curl
> (upload→download bytes→move→delete + on-disk cleanup); frontend `tsc`/`eslint`/`vitest`
> (26)/`build` + live screenshot. Hardened via adversarial review (57 findings → 44 verified →
> ~17 applied, incl. a stored-XSS fix on the content path). Deferred: streaming upload `put`,
> transactional delete/move under concurrency, S3 backend, move-via-UI, pagination UI.

## Phase 5 — Idea management & organization

Goal: capture, tag, link, and organize ideas/notes.

- [x] **5.1** `idea` table (migration `0003`): title, Markdown `body`, `tags[]`, `status`, `links[]` (undirected uuid refs), timestamps; owner + status indexes. (Links via a uuid array kept symmetric by the app, not graph edges — pragmatic for the uuid-public-id model.)
- [x] **5.2** Owner-scoped CRUD; list filtered by status/tag + case-insensitive title/body search + pagination; distinct-tags endpoint; symmetric link/unlink with link-scrub on delete; detail resolves related ideas. Title/status/tags validated.
- [x] **5.3** Frontend: Ideas page with **list + board** (by status) views, search + status + tag filters; editor modal with tag chips, status, and **linked-ideas** management (add via search picker, unlink). Route is code-split (lazy).
- [x] **5.4** Reusable `MarkdownEditor` (Write/Preview tabs, `react-markdown` + GFM, XSS-safe) + `prose` styling via `@tailwindcss/typography`; shared with the Phase 7 document editor.
- [x] **5.5** Tests: **5 backend** integration (CRUD, filters/search, tags, symmetric link/unlink + delete-scrub, validation, repo-level owner isolation) + **30 frontend** (incl. IdeasPage list/board + MarkdownEditor).

> Verified green: backend `build`/`clippy -D warnings`/`test` (35)/`fmt` + live curl
> (create→filter→search→tags→link→detail→delete-scrub); frontend `tsc`/`eslint`/`vitest`
> (30)/`build` + live screenshot of the Ideas page. Bundle code-split so the Markdown
> libs load only with the Ideas route. Adversarial review workflow not run (offer pending).

## Phase 6 — LLM provider abstraction & AI integration

Goal: a vendor-neutral AI layer; analysis/insights over the user's data.

- [x] **6.1** Provider-neutral async `LlmProvider` trait (`chat_stream`, `models`, `requires_key`, modalities) + an `LlmRegistry`; shared SSE byte-stream parser.
- [x] **6.2** Adapters: **Mock** (offline, no key — the verified path), **OpenAI-compatible** (OpenAI + OpenRouter), **Anthropic** (real adapters implemented against documented APIs; not exercised in CI — no keys/egress here). Per-owner API keys **encrypted at rest** (ChaCha20-Poly1305, key derived from `APP_SECRET`; `provider_key` table, migration `0004`).
- [x] **6.3** Static model registry per provider; `GET /ai/providers` lists models + a per-owner `configured` flag; chat routes by `provider`+`model`. (Usage/cost capture deferred.)
- [x] **6.4** Streaming via **SSE** (`POST /ai/chat`); frontend reads the stream with `fetch` + a ReadableStream SSE parser.
- [ ] **6.5** ~~Multi-modal (image/audio/video) + fal.ai~~ — **deferred** (text chat only for now; the trait carries modality metadata).
- [~] **6.6** chat-with-your-data via optional `context` grounding injected into the system prompt. Full RAG (embeddings + SurrealDB vector search) and summarize-a-file/idea surfaces **deferred**.
- [x] **6.7** Frontend AI page: provider/model picker, streaming chat (Markdown-rendered replies, Stop), and encrypted key management (`KeySettingsModal`). Route code-split (lazy).
- [x] **6.8** Tests: **backend** crypto (roundtrip/wrong-key/tamper), key store (owner-scoped, encrypted, never leaked), providers list, mock chat SSE, chat validation; **frontend** `streamChat` SSE parser + AiPage. Guardrails: key length/validation, message-size cap, timeouts inherited from request handling. Rate limiting deferred to Phase 10.

> Verified green: backend `build`/`clippy -D warnings`/`test` (**44**)/`fmt` + live curl
> (providers, mock SSE chat, key CRUD); frontend `tsc`/`eslint`/`vitest` (**34**)/`build`
> + live screenshot of the AI page. **No LLM egress/keys in this env** — real adapters
> compile and are wired but unexercised; the Mock provider is the end-to-end-tested path.
> `APP_SECRET` must be set in production (a dev default is used otherwise, with a warning).

## Phase 7 — HTML document editor & conversion/export pathway

Goal: rich document editing plus the shared HTML ↔ Markdown ↔ PDF ↔ Office pipeline.

- [x] **7.1** **TipTap** rich-text editor (StarterKit); `document` table (migration `0005`): HTML `body`, `version` (bumped per save), owner, timestamps. Body sanitized with `ammonia` on store.
- [x] **7.2** Editor UI: formatting toolbar (bold/italic/strike, H1/H2, lists, quote, code, undo/redo), Markdown **import** (.md → HTML) + **export** menu, debounced **autosave** with status, document list (create/select/delete).
- [x] **7.3** **Shared conversion module** (`src/convert.rs`, single pathway): Markdown↔HTML (`pulldown-cmark` / `html2md`), `ammonia` sanitize, and export to:
  - [x] PDF — headless Chrome (`CHROME_BIN`), verified end-to-end.
  - [~] MS Word (`.docx`) — via Pandoc (`PANDOC_BIN`) when installed; returns a clear 503 otherwise (Pandoc absent here). **Excel/PowerPoint deferred** (those target structured data, not prose docs).
- [x] **7.4** Export endpoints: `GET /documents/:id/export?format=…` and the shared `POST /export` (arbitrary content). (Async/progress for large jobs deferred.)
- [x] **7.5** Reusable `ExportMenu` (PDF / Word / HTML / Markdown) calling `POST /export` and downloading the blob — usable by any feature (documents pass HTML, ideas could pass Markdown).
- [x] **7.6** Tests: **backend** doc CRUD + owner isolation, md↔html conversion, sanitize, export to markdown/html, **PDF via Chrome** (asserts `%PDF`), docx-when-absent (503), unsupported-format (400); **frontend** DocumentsPage + ExportMenu.

> Verified green: backend `build`/`clippy -D warnings`/`test` (**52**)/`fmt` + live curl
> (CRUD, sanitize, export md/html/**pdf**, docx→503); frontend `tsc`/`eslint`/`vitest`
> (**36**)/`build` + live screenshot of the editor. Editor route is code-split (TipTap is
> its own ~120 KB gz chunk). **SSRF note:** server-side Chrome renders user HTML — sanitized
> with ammonia (scripts/handlers stripped); network-isolating Chrome is a Phase-10 hardening item.

## Phase 7.5 — MCP server (foundation, shipped this session)

Goal: expose Baitler's existing features as Model Context Protocol tools so external agents
(Claude Code, Grok Code, Hermes) drive the same owner-scoped repos a REST caller would.

- [x] **7.5.1** Built-in MCP server at `POST /mcp` served **in-process** by `baitler-api` (Streamable HTTP, JSON-response variant; batched arrays; `GET`/`DELETE /mcp` → `405`; no server-initiated SSE). Code in `backend/crates/api/src/mcp/{mod.rs,protocol.rs,tools.rs,b64.rs}`; reuses the same repos + DB, so MCP and REST never drift.
- [x] **7.5.2** `baitler-mcp` stdio↔HTTP bridge binary (`backend/crates/api/src/bin/…`) for stdio-only clients; a thin forwarder that never opens the RocksDB file (no lock contention with the API).
- [x] **7.5.3** **24 tools**: `health`; `ideas_{list,get,create,update,delete,link,unlink,tags}`; `documents_{list,get,create,update,delete,export}`; `files_{list,get,read,write,delete}`; `folders_create`; `ai_{providers,chat}`; `export`. Binary results (pdf/docx/file reads) returned Base64 in JSON; `MAX_BLOB` (24 MB) caps reads/writes over MCP.
- [x] **7.5.4** `MCP_ENABLED` toggle and optional `MCP_AUTH_TOKEN` bearer auth (constant-time compare, never logged); `initialize` echoes the client's protocol version.
- [x] **7.5.5** Install guide `docs/mcp.md` (HTTP + stdio for Claude Code, Hermes, generic clients) and the drift guards `every_advertised_tool_is_dispatchable_by_name` (hardcoded name list + exact-count assert) and `tool_schemas_are_well_formed`.

> Shipped and merged to `main` this session. All tools resolve the single `DEV_OWNER` via the
> `owner.rs` stub — Phase 2 auth swaps only the `CurrentOwner` extractor and every tool inherits
> per-user scoping with no client change. Phase 11 builds **on** this; it does not re-create it.
> Two things stay strictly in sync with every new tool, in the same commit: the `call()` dispatch
> match **and** the `known` name list + `assert_eq!(advertised.len(), known.len())` in
> `mcp/tools.rs` — adding a tool without both turns `cargo test` red.

## Phase 8 — Data visualization & analytics

Goal: charts and dashboards over the user's files, ideas, and AI usage.

- [ ] **8.1** Analytics endpoints: aggregate counts, activity over time, AI usage/cost, storage usage.
- [ ] **8.2** Choose charting lib (Recharts/visx/ECharts); reusable chart components.
- [ ] **8.3** Analytics dashboard in the base portal; filters and date ranges.
- [ ] **8.4** Optional: user-defined dashboards / saved views.

## Phase 9 — Mobile apps (iOS & Android)

Goal: native clients consuming the same API.

- [ ] **9.1** Decide native vs. shared (brief says native iOS + Android; confirm before investing).
- [ ] **9.2** iOS (Swift/SwiftUI) project: OAuth login, base portal, files, ideas, AI chat.
- [ ] **9.3** Android (Kotlin/Compose) project: same feature parity.
- [ ] **9.4** Shared API contract (OpenAPI spec generated from backend) to keep clients in sync.
- [ ] **9.5** Mobile build scripts under `scripts/`.

## Phase 10 — Hardening, deployment & polish

Goal: production-ready.

- [ ] **10.1** Security pass: secrets management, encrypted API keys, input validation, rate limiting, CORS/CSRF, dependency audit.
- [ ] **10.2** Observability: structured logs, metrics, error tracking.
- [ ] **10.3** Containerization (`Dockerfile` per service) + `docker-compose` for full local stack incl. SurrealDB.
- [ ] **10.4** Deployment target (baitler.com): hosting, TLS, env config, DB backups/migrations.
- [ ] **10.5** End-to-end tests (Playwright) over critical flows: login → upload → edit → export → AI insight.
- [ ] **10.6** Performance pass (bundle size, query/index tuning in SurrealDB, caching).
- [ ] **10.7** Docs: user guide + developer onboarding in `docs/`.

## Phase 11 — Agentic Baitler (knowledge base & assistant)

Goal: turn the shipped MCP surface into a trustworthy agentic loop — an external agent organises a
project's knowledge in Baitler, publishes human-readable/exportable artifacts, and answers questions
grounded in the stored data, while the human stays in control via a Projects view, an activity audit
timeline, and a draft→published review gate.

Vision: "Document the projects I'm working on into Baitler, making it my personal knowledge base and
assistant." Through `/mcp`, agents (1) organise the knowledge base, (2) generate web pages / Markdown /
PDF / Office so knowledge is human-readable and exportable, and (3) read the data to answer questions and
produce content. The smallest change that makes this real is two additive primitives layered onto the
existing owner-scoped model — a `project` grouping with cross-type membership, and an append-only
`activity` log tagged with the calling agent — plus full-text search, a single attribution seam threaded
through the MCP layer, and a publishing step that reuses `convert.rs`. RAG (embeddings/vector search),
multi-agent IAM, and any public web surface are deliberately deferred behind real need or Phase 2 auth.

Architecture: everything is purely additive — new module `knowledge/` (project + membership + activity
+ search repos), migrations `0006`–`0008`, and new MCP tools that mirror the `ideas_*`/`documents_*`
style. No existing table or repo is rewritten; the one cross-cutting change is threading an `Actor` (owner
+ agent label) from `mcp::handle` down to `tools::call`. A new `review` field (NOT the existing closed
`status` enum) carries draft/published. Retrieval uses SurrealDB full-text `SEARCH` indexes over existing
prose; the `ai_chat` `context` fold already covers grounded answering. Milestones: A (MVP loop + search +
activity), B (publishing), C (MCP discoverability: prompts/resources), D (multi-agent identity, gated on
Phase 2). Each milestone is independently shippable.

### A — Knowledge model, search, activity, MVP loop

- [x] **11.1** Capability gate (blocking, do first): with a throwaway query against both the `kv-mem`
  (tests) and `kv-rocksdb` (dev) engines, confirm the pinned `surrealdb` 2 build executes
  `DEFINE ANALYZER … TOKENIZERS class FILTERS lowercase,ascii` and `DEFINE INDEX … SEARCH ANALYZER … BM25`
  over an already-populated table inside a migration's `BEGIN/COMMIT`. Block 11.4/11.5 on this; if `SEARCH`
  is unavailable on the embedded engine, fall back to the existing `string::lowercase(...) CONTAINS`
  approach (as `ideas_repo::list_ideas` already does) and drop the analyzer.
- [x] **11.2** Migration `0006_knowledge.surql` (SCHEMAFULL, mirroring `0003_ideas.surql`). Define `project`:
  `uuid` (UNIQUE index `project_uuid`), `owner`, `name`, `slug` (composite UNIQUE `project_slug` ON owner,slug),
  `summary` (Markdown DEFAULT ""), `status` DEFAULT "active" (active|archived), `created_at` DEFAULT
  `time::now()`, `updated_at` VALUE `time::now()`; index `project_owner` ON owner,status. Define one polymorphic
  edge table `kn_member` serving both project membership and cross-type links: `uuid` UNIQUE, `owner`,
  `kind` (member|link), `src_type`/`src_id`, `dst_type`/`dst_id` (`*_type` ∈ idea|document|file|project),
  `relation` DEFAULT "" (free-form: contains|implements|supersedes|references), `created_at`; indexes
  `kn_member_owner` ON owner,kind and `kn_member_src` ON owner,src_type,src_id and `kn_member_dst` ON
  owner,dst_type,dst_id. Files are members-only (no file↔file links); constrain `dst_type` validation to the
  four types that matter.
- [x] **11.3** Add a `review` field (draft|published, DEFAULT "published") to `idea` and `document` in the same
  migration, plus optional `project_id` (`option<string>`, DEFAULT NONE) on `idea`, `document`, and `file`.
  Existing rows default `review="published"` so nothing already captured is hidden; agent writes pass
  `review="draft"` explicitly, human/portal writes default to published. Do NOT overload the existing closed
  `idea.status` enum (`inbox|active|done|archived`, validated by `clean_status`, indexed by `idea_owner`) — it
  stays untouched. Add the new fields to the SCHEMAFULL migration, the `CREATE CONTENT`/`UPDATE SET` builders in
  `ideas/repo.rs` and `documents/repo.rs`, and `IdeaDto`/`DocumentDto`. Add a `document_review` index for the
  review-queue/publish queries.
- [x] **11.4** Migration `0007_search.surql`: a `kn_text` analyzer and `SEARCH` (BM25) indexes over already-existing
  prose — `idea(title,body)`, `document(title,body)`, `project(name,summary)`, `file(name)` — gated on 11.1. No
  schema change to the indexed tables. Add `doc_repo::search_documents` and `knowledge::repo::search` running a
  per-type `@@`/`search::score()` query per table and returning **typed sections** each internally ranked, ordered
  overall by a stable secondary key (`updated_at`); do not promise a fake unified cross-table relevance rank.
  Snippets via `search::highlight()` or a manual substring around the match.
- [x] **11.5** New `knowledge/` module (`mod.rs,model.rs,repo.rs,routes.rs`) following `ideas/` verbatim. Project
  CRUD (`PROJECT_SELECT` projection, owner-scoped, slug generated from `name` with `-2` collision suffix).
  Membership/link repo: `add_member`/`remove_member`/`list_members(project, type?)`; `link_items`/`unlink_items`
  written as BOTH directed `kn_member kind='link'` rows (mirroring `link_ideas`' symmetric write — never a
  union-query path, which would defeat `kn_member_dst`); `backlinks(type,id)` querying src OR dst. Validate that
  **both** endpoints exist and are owner-owned before inserting an edge (mirror `ideas_link`'s existence checks).
  Wire a `kn_member` scrub into `delete_idea`/`delete_document`/`delete_file` (and `delete_project` removes
  memberships only, never the underlying items) — mirroring the Phase 5 link-scrub-on-delete precedent.
- [x] **11.6** Thread attribution: resolve an `Actor { owner, agent: Option<String> }` once in `mcp::mod.rs::handle`
  (the only function with headers) and pass it as a parameter through `dispatch` → `handle_tools_call` →
  `tools::call(state, &actor, name, args)`, changing those signatures and the per-tool fns to read `&actor.owner`
  in place of the `DEV_OWNER` literals. The default for the open/legacy path is `Actor { owner: DEV_OWNER,
  agent: None }`, so the refactor is behaviour-preserving. The agent label comes from an optional `X-Baitler-Agent`
  header (or a per-token label once 11.13 lands) — no agent table required for v1. This single seam is the
  prerequisite for activity attribution and the Phase 2 owner swap.
- [x] **11.7** Migration `0008_activity.surql`: append-only `activity` table — `uuid` UNIQUE, `owner`, `agent`
  (`option<string>`, NONE = human/web), `action` (e.g. idea.create, document.publish, file.delete), `target_type`,
  `target_id`, `target_title`, `project_id` (`option<string>`), `summary`, `created_at`; index `activity_owner` ON
  owner,created_at. A central helper appended at the end of each **mutating** write tool
  (`ideas_{create,update,delete,link,unlink}`, `documents_{create,update,delete}`, `files_{write,delete}`,
  `folders_create`, `projects_*`); read tools log nothing. Wrap the content write + activity insert in one
  SurrealDB transaction so "exactly one activity row per write" holds under failure (else document it as
  best-effort). Never persist tool argument bodies verbatim — a derived summary + target id only.
- [x] **11.8** New MCP tools (each owner-scoped via `&actor.owner`, mirroring `ideas_*` validation helpers):
  `projects_{list,get,create,update,delete}` (`projects_get` resolves member counts + typed summaries);
  `projects_add_item`/`projects_remove_item` (set/clear membership for an idea/document/file); `knowledge_link`/
  `knowledge_unlink` (generic cross-type edge); `knowledge_backlinks`; `knowledge_search` (typed, ranked hits
  across idea/document/project/file with id/type/title/snippet — the agent's "access the data" entry point);
  `activity_list` (filter by project/agent/since). Extend existing `ideas_{create,update}` and
  `documents_{create,update}` schemas with optional `project_id` and `review` args. Update the `call()` match, the
  `known` name list, and the `assert_eq!` count in `mcp/tools.rs` in the same commit.
- [x] **11.9** Offline end-to-end acceptance test driving the JSON-RPC surface as an agent would, no egress:
  `initialize` → `tools/list` → `folders_create` → `files_write` → `ideas_create` (review=draft) →
  `projects_create` + `projects_add_item` → `documents_create` (review=draft) → `knowledge_link` →
  `knowledge_search` (asserts the seeded items rank) → `documents_export` format=markdown/html (asserts bytes) →
  `ai_chat` against the `mock` provider grounded via `context`. Cap assembled `context` (bound the number of
  sources and per-source chars) before folding into `build_system`. PDF/DOCX assertions stay conditional on
  `CHROME_BIN`/`PANDOC_BIN`, as the Phase 7 tests already are.

> **Milestone A shipped** (branch `phase-11-agentic`, PR #1; backend green: 94 tests, clippy/fmt).
> As-built deltas from the design above: membership is the direct `project_id` pointer + a dedicated
> **`kn_link`** link table (instead of one `kn_member` table with `kind ∈ {member,link}`) — simpler,
> indexed member queries, one link system. The FTS analyzer and SEARCH indexes are split across
> migrations `0007`/`0008` (the runner wraps each file in one transaction and an index can't reference
> an analyzer defined in the same uncommitted txn — verified with throwaway probes); activity is
> migration `0009`. `knowledge/routes.rs` (REST) is deferred to 11.12 with the portal; the MCP tools
> are the agentic interface. Activity logging is best-effort (not in the write's transaction).

### B — Publishing & export

- [x] **11.10** SSRF hardening of the server-side renderer (independent, highest priority — exploitable **today**
  via `documents_export(pdf)` and `POST /export`): network-isolate headless Chrome in `convert.rs::html_to_pdf`
  (run in a network namespace / firewalled container blocking egress except loopback-none; drop `--no-sandbox`
  where the host allows a real sandbox), and add a stricter publish-profile sanitizer that strips remote resource
  loads (external `<img>/<iframe>/<link>`, `srcset`, oversized `data:` URIs) before any render. Document
  `CHROME_NETNS`/sandbox env in `.env.example`. Keep the existing 30s timeout + output-size cap.
- [x] **11.11** `documents_publish` and `collection_export` reusing the shared pathway: render a document (or a
  project's ordered member documents concatenated with a title page + combined TOC) to a self-contained, sanitized
  HTML / Markdown / PDF / DOCX artifact via `convert::export` (unchanged), and persist it as an owner-scoped file —
  mirroring `files_write`'s full pattern (`storage.put(key, bytes)` AND `files_repo::create_file(...)` with
  storage-rollback-on-failure), returning a stable file id. Publishing a document flips its `review` to published.
  Run synchronously inside the request/tool call (the only slow path is one Chrome invocation, already bounded to
  30s) — no job queue, no progress notifications (the JSON-response transport can't push, and an async jobs layer is
  over-engineering for a single user). Excel/PowerPoint stay deferred (structured data, not prose).
- [x] **11.12** Frontend (thin): a Projects page (project cards with member + draft-pending counts) and a Project
  detail view that lists member documents/ideas/files grouped by type, each linking to its existing Files/Ideas/
  Documents feature page, with a provenance line ("created by claude-code") and a Draft/Published chip; a Review
  queue filtering `review=draft` ideas+documents with inline Approve (PATCH → published) and Reject/Delete (reuse the
  Phase 4 accessible ConfirmModal); an Activity timeline (`GET /activity`). Reuse the existing `MarkdownEditor` and
  `ExportMenu` (a project README exports via `POST /export`). Lazy-loaded and nav-registered like the other features.

> **Milestone B shipped** (branch `phase-11-agentic`). 11.10 SSRF-hardens the renderer
> (`harden_for_render` strips remote image sources; Chrome resolves no hostnames; `--no-sandbox`
> gated on `CHROME_NO_SANDBOX`; Pandoc `--sandbox`). 11.11 adds `documents_publish`/`collection_export`
> (38 MCP tools). 11.12 ships `knowledge/routes.rs` (REST: projects CRUD + membership + `GET /knowledge/search`,
> `/review`, `/activity`; `review` accepted on PATCH /ideas + /documents) and a lazy `ProjectsPage` portal
> (Projects/Review/Activity tabs, provenance badges). Verified: backend 98 tests + clippy/fmt; frontend
> `tsc`/`eslint`/`vitest` (39) + `vite build`. Deferred: a live browser screenshot; logging human/portal
> actions to `activity` (today only MCP agent actions are recorded); standalone link REST (links via MCP).

### C — MCP discoverability (additive protocol surface)

- [ ] **11.13** Advertise and implement prompts: add `"prompts": { "listChanged": false }` to the `initialize`
  capabilities, implement the `prompts/get` dispatch arm (only `prompts/list` exists today, stubbed to `[]`;
  `prompts/get` currently returns method-not-found), and ship server-side templates encoding the house workflows:
  `document_project`, `organise_inbox`, `answer_from_kb`, `publish_document`. Each renders a `messages` array with
  live repo context substituted for the given ids. Enrich the `initialize` `instructions` string with the
  organise→publish→retrieve loop so a cold agent does the right thing.
- [ ] **11.14** Advertise and implement resources (optional, only if a consuming client needs it): add
  `"resources": { "subscribe": false, "listChanged": false }` to capabilities, implement `resources/list` +
  `resources/read` (both stubbed/absent today) exposing a `baitler://{idea|document|project}/{uuid}` URI scheme over
  the existing getters (text resources as `{uri,mimeType,text}`; honor `MAX_BLOB`), plus
  `resources/templates/list`. Owner-scoped via `&actor.owner`, capped + reusing `knowledge_search` for a
  `baitler://search?q=` template. Defer subscriptions, `outputSchema`/`structuredContent`, and MCP cursor pagination
  — `limit`/`offset` is plenty for a personal KB.

### Notes

> Acceptance: an agent issuing one instruction ("document this repo as a Baitler project") produces a `project`
> row, ≥1 `document` and ideas bound via `project_id`/`kn_member`, cross-links resolvable from either endpoint and
> scrubbed on delete, and matching `activity` rows tagged with the agent — all owner-scoped and visible in the
> portal. `knowledge_search` returns typed, ranked hits across idea/document/project/file (works without any
> embeddings). Agent writes land as `review="draft"` in the Review queue; Approve flips them to published; existing
> content is never hidden by the migration. `documents_publish`/`collection_export` reuse the single `convert.rs`
> pathway and persist owner-scoped files (PDF needs Chrome, DOCX needs Pandoc; both degrade to a clear 503).
> Backend `build`/`clippy -D warnings`/`test`/`fmt` and frontend `tsc`/`eslint`/`vitest`/`build` green; the Mock
> LLM is the asserted path; no test needs egress or keys; the vector/SEARCH DDL is smoke-tested under `kv-mem` in
> CI; the `mcp/tools.rs` drift guard (name list + count) is updated in lockstep with every new tool.
>
> Deferred (build only on real need): **RAG** — `kn_embedding` table, HNSW vector index, `kb_search`/`kb_recall`,
> an `embed()` capability on `LlmProvider`, a Mock embedder, and `EMBEDDINGS_*` config (full-text `SEARCH` + the
> existing `ai_chat` context fold already satisfy "answer questions about my data" for one user); a first-class
> `note` type (model a note as an idea with `status="inbox"`); a public unauthenticated `/p/:slug` web surface; an
> async jobs/progress layer and MCP `notifications/progress` (the transport can't push); MCP resource subscriptions,
> `outputSchema`/`structuredContent`, and cursor pagination; per-agent IAM (token table, scopes, capability matrix,
> rate/quota budgets).
>
> Depends on Phase 2 auth: today every tool resolves `DEV_OWNER` and the `agent` label is the only attribution
> dimension (the human/agent distinction is meaningless pre-auth). When OAuth lands, the `Actor` seam (11.6) swaps
> `DEV_OWNER` for the session owner in one place, agent tokens become user-issued credentials, and any public
> publishing surface gains real ownership — with no knowledge-layer query changes. The public `/p/:slug` page, true
> multi-agent identity, and per-token scopes are sequenced **after** Phase 2; until then publishing means
> "downloadable owner-scoped file in the portal," and the new REST routes inherit the same localhost-only-until-auth
> caveat as the rest of the API.

## Phase 12 — Web page hosting (publish pages at a URL)

Goal: turn Baitler's Phase 11 "publishing = a downloadable owner-scoped file" into the deferred **URL-addressable served page** — author a page from Markdown/HTML, publish it, and share it via a stable public link `GET /p/{slug}`, with pages organised in folders, searched, and filtered.

Vision: "Host web pages on Baitler." A page is authored like a Phase 7 document (TipTap HTML or imported Markdown, sanitized through the one `convert.rs` pathway) and filed in the Phase 4 folder tree, but unlike a document it can be **published to a served URL** rather than only exported to a file. Pages reuse everything already built — `convert.rs` (`md_to_html`/`sanitize`/`harden_for_render`), the Phase 4 `folder` hierarchy and the secure-download header posture, the Phase 11 `kn_text` full-text search and `activity` provenance log, the MCP `Actor`/owner seam and tool/drift-guard conventions — so this phase is **additive**, not a re-implementation. The agentic loop the Phase 11 brief called for ("agents make web pages so knowledge is human-readable") gets its natural endpoint: an agent authors a page over MCP and `pages_publish` hands back a shareable URL.

Architecture: one new `page` table — a document-like row distinct from `document` (it adds a `slug`, a `visibility` draft|unlisted|public gate, a `source_format`, a `folder_id` reusing the Phase 4 `folder` tree, and a `project_id` membership pointer; it is **not** a `document` with extra flags, and **not** a new folder system) — a new `pages/` module mirroring `documents/`/`knowledge/` verbatim (model/repo/routes), a new **public render module** that serves the stored sanitized HTML at `GET /p/{slug}`, additive `pages_*` MCP tools, a page SEARCH index pair folded into `knowledge_search`, and a lazy `PagesPage` portal. No existing table or repo is rewritten; the only edits to existing code are additive (router wiring, the `knowledge` whitelists, a `delete_page` link scrub, two `activity::entry_for` arms, and lifting the slug helpers — see 12.1). **Decisions taken up front:** (a) a page is a *separate table from `document`*, sharing only `convert.rs`, because their lifecycles diverge (a document is a private, export-oriented editing workspace with a per-save `version`; a page is a slug-addressed, publishable, visibility-gated artifact) — coupling them would bloat `document` with publish-only columns and entangle its version logic; a one-way `from_document` promote path is the only bridge. (b) Pages *reuse the Phase 4 `folder` table* via `folder_id` exactly like `file.folder_id` — one shared owner-scoped hierarchy with the existing breadcrumbs/move/cycle-guards, no second tree and no "site" container (a multi-page grouping, if ever needed, is a Phase 11 `project`, not a new entity).

### Security reality (read before B) — this is the whole risk of the phase

- **Serving user/agent-authored HTML at a public URL is the headline risk.** A served page is attacker-controllable HTML, and an agent can author a *malicious draft* (over `pages_create`) before any human looks. The stored `body` is sanitized with `convert::sanitize` (ammonia, scripts/handlers stripped) on **every write** — the same invariant documents already hold, so an owner-facing preview never renders unsanitized markup — and the public serve path **additionally** runs `convert::harden_for_render` over the body so remote `<img>`/`srcset` and resource-loading tags are stripped before the bytes reach a viewer. Defense-in-depth is mandatory because once Phase 2 ships cookies, an XSS on a same-origin page steals the session.
- **Origin isolation + a no-script CSP are the load-bearing controls, not the sanitizer alone.** `convert::sanitize` keeps `<a href>` and (without harden) http(s) `<img>`; the durable protection is **serving `/p/*` from a separate origin** (a distinct `PUBLIC_PAGE_ORIGIN`, e.g. `*.pages.baitler.com` or a cookie-path-isolated host) so a page can never read the app's auth cookie, **plus** a strict response CSP whose `default-src 'none'` blocks scripts even if a sanitizer bypass ever lands. The `sandbox` directive (no `allow-scripts`, no `allow-same-origin`) already forces each page into an opaque, cookieless origin in the browser — so same-origin serving is *survivable* pre-auth, but the separate origin is a **hard Phase-2 blocker**: **Phase 2 must not set an auth cookie on any origin that also serves `/p/*`.**
- **SSRF is a viewer-side, not server-side, concern here — and is already solved upstream; reuse it.** There is **no server-side headless render on the serve path** (we serve stored HTML, never a per-request Chrome render), so the serve route adds zero server-side SSRF surface. `harden_for_render` at serve time stops a *viewer's* browser from fetching an internal-host `<img>` an agent embedded. The only server-side render in a page's life is at publish-time PDF/DOCX export, which already inherits `convert.rs`'s Chrome DNS-block + sandbox hardening (Phase 11.10) and is owner-triggered, never anonymous.
- **`data:` URIs and `<a>` links are the two residual content vectors the CSP must backstop.** `harden_for_render` keeps `data:` image URIs (so authors inline images) and keeps `<a href>`. A `data:image/svg+xml` does not execute in an `<img>` context, and `default-src 'none'; sandbox` neutralizes script regardless — but the CSP is the backstop, so it is asserted byte-for-byte in a test. `<a>` links survive, so a published page is inherently a possible phishing/redirect host; for a single-owner personal app this is an accepted risk (the owner authors their own pages), called out rather than over-engineered away.
- **No CSRF/auth surface on the public GET.** `/p/{slug}` is a pure read of `visibility ∈ {unlisted, public}` rows; it takes no cookies, no `Authorization`, and must **not** inherit the credentialed CORS layer. Today `routes/mod.rs` applies one `CorsLayer` (`allow_credentials(true)`) via `.layer(cors)` over the *whole* router — so the public router is a concrete wiring change, built as its own `Router` and merged **outside** that layer and outside any future Phase 2 auth middleware (see 12.5). The `CurrentOwner` extractor is intentionally absent from the serve handler, so the owner-less public read can never silently scope to `DEV_OWNER`.
- **Slug enumeration / privacy.** `unlisted` = link-knowers only (unguessable slug, `noindex`); `public` = also indexable. Slugs are owner-scoped-unique; for `unlisted`, the slug generator appends entropy so links aren't guessable. `draft`/missing both return **404** (the codebase has no 403 variant, so this is automatic) so existence is never confirmed. Send `X-Robots-Tag: noindex` for `unlisted`. `unlisted` is *obscurity, not access control* (the URL lands in browser history) — stated plainly.
- **Pre-auth caveat (state it crisply).** Until Phase 2, every authoring tool resolves the one `DEV_OWNER`, so "shared with others" means "anyone with the link can view"; there is no per-recipient ACL and no second user to share *to*. **`filter by author` is near-trivial today — there is exactly one owner** — so the list endpoint *accepts* the `author` param but it is a documented no-op until Phase 2; **no `created_by` column is added** (SurrealDB is SCHEMAFULL with `DEFINE FIELD … IF NOT EXISTS`, so the auth phase adds it in one line when it does something).
- **Over-engineering to avoid (personal app):** a published-snapshot column (`published_html`/`published_version`), a `share_token` + rotation lifecycle, a `site`/template/theme engine, a `page_asset` table + asset-serving route (images are inlined `data:` URIs — `harden_for_render` guarantees no `<img src='self'>` can ever be populated), a `tags`/`created_by` column, an in-process per-IP rate limiter or `AppError::TooManyRequests` (rate limiting stays a Phase 10 item), ETag/CDN/edge caching, page versioning/history, async publish jobs, and an old-slug→new-slug redirect table. Serve synchronously from the DB; a stored-HTML read + one `harden_for_render` pass (string-level, bounded by the `MAX_BODY` write cap) is fast.

### A — Page model + authoring + owner-only management (ships now, no public surface)

- [x] **12.1** Prerequisite refactor + migration `0010_pages.surql` (SCHEMAFULL, mirroring `0005_documents.surql` + `0006`'s slug conventions). First, **lift the slug helpers out of `knowledge/repo.rs`**: make `slugify` `pub` (default fallback word parameterized, not hardcoded `"project"`) and generalize `unique_slug` to take a table name (it currently hardcodes `FROM project`) — move both into a small shared `src/slug.rs` (or `pub`-export them) so projects and pages share one collision-suffix generator; this is a real edit to `knowledge/repo.rs`, owned here. Then define `page`: `uuid` (UNIQUE `page_uuid`), `owner`, `title`, `body` (sanitized HTML, DEFAULT ""), `slug` (composite UNIQUE `page_slug` ON owner,slug), `visibility` DEFAULT "draft" (draft|unlisted|public), `source_format` DEFAULT "html" (html|markdown — remembered so the editor round-trips), `folder_id` `option<string>` DEFAULT NONE (**reuses the Phase 4 `folder` table** — no new folder type), `project_id` `option<string>` DEFAULT NONE (Phase 11 membership pointer, so a page can join a project and be cross-linked via `kn_link`), `version` int DEFAULT 1, `published_at` `option<datetime>` DEFAULT NONE, `created_at`/`updated_at`. Indexes: `page_owner` ON owner,visibility, `page_folder` ON owner,folder_id, `page_project` ON owner,project_id. Add `page` to the `MEMBER_TYPES`/`ITEM_TYPES` whitelists in `knowledge/model.rs` and extend `set_membership`/`project_members`/`delete_project`'s member loop so pages are first-class in projects/links/scrub-on-delete (without this, `scrub_item_links(…, "page", …)` fails the `any_table` whitelist). Extend `folder_is_empty` to also count pages so a folder holding a page can't be wrongly deleted.
- [x] **12.2** New `pages/` module (`mod.rs,model.rs,repo.rs,routes.rs`) following `documents/` verbatim where it overlaps and owning the net-new surface where it doesn't (documents have **no** slug/visibility/publish — slug generation, owner-unique collision handling, the visibility state machine, and publish/unpublish are new logic). `PageRow`/`PageDto`/`PageSummary` (the summary omits `body` for payload size; `PageDto` carries `public_url` — populated by the serving facet from `PUBLIC_PAGE_ORIGIN` when published, empty for drafts — so the frontend/agent get a usable share link); const slices `SOURCE_FORMATS`/`VISIBILITIES` for literal validation (mirroring `PROJECT_STATUSES`). Owner-scoped CRUD with a `PAGE_SELECT` projection; slug generated from `title` via the lifted `slugify`+`unique_slug` (with extra entropy appended for `unlisted`, see 12.6); **body sanitized via `convert::sanitize` on every write** (same as documents — so any draft preview is already safe); Markdown `source` paths through `convert::md_to_html` before `sanitize`; `version` bumps on a content edit but not a visibility-only change (mirror `documents/repo.rs`); a `MAX_BODY` (5 MB, mirroring `documents/routes.rs`) cap on writes. `create_page` accepts an optional `from_document` (copies a document's sanitized body into a new `source_format='html'` page — the one-way promote bridge, no second editor). `get_page_by_slug` (owner-scoped variant; the serving facet adds the unauthenticated one). Wire a `kn_link` scrub into `delete_page` (mirror `delete_document`).
- [x] **12.3** Owner-scoped REST in `pages/routes.rs`, merged into the **auth-gated** router tree (behind `CurrentOwner`, alongside `/documents`): `GET/POST /pages`, `GET/PATCH/DELETE /pages/{id}`, `POST /pages/{id}/publish` (sets `visibility`, stamps `published_at`, returns the page DTO **with its absolute `public_url`**), `POST /pages/{id}/unpublish` (→ draft, immediately 404s `/p/{slug}`). `PATCH` accepts `title`/`body`/`source_format`/`slug`/`visibility`/`folder_id`/`project_id` (a custom `slug` is re-validated to the slugify charset and re-checked owner-unique; visibility folds in here, mirroring how `documents_update` takes `review` inline — no separate `/visibility` endpoint). Listing supports `folder`, `visibility`, `q` (case-insensitive name/body `CONTAINS` fallback like `ideas_repo::list_ideas`), `project`, and `updated_at` ordering with pagination — mirroring the `files` list params (`MAX_LIMIT`/`DEFAULT_LIMIT` from `files/routes.rs`). The `author` filter param is accepted but resolves to the one owner pre-auth (documented Phase-2 no-op; no new column). Reuse the Phase 7 `POST /export` for "export this page as PDF/Word/MD" — no new export code.
- [x] **12.4 (tests-with-features)** Backend integration `tests/pages.rs`: page CRUD + owner isolation (two synthetic owners, mirror `documents.rs`/`files.rs`), slug uniqueness + collision suffix + custom-slug re-validation, `sanitize` strips a `<script>`/`onerror` on write (asserted on the stored `body`), visibility transitions (draft→public→unlisted→draft), `version` bumps on a content edit but not a visibility flip, folder filing + folder filter + folder-not-empty-with-page, `q` search, `from_document` promote copies the body, project membership + `kn_link` scrub-on-delete, and the `public_url` field on publish. All on the embedded `kv-mem` engine, ephemeral ports — no egress.

> **Milestone A shipped** (branch `phase-12-web-hosting`, commit `3df4a04`). Migration `0010_pages.surql`
> adds the `page` table (slug/visibility/source_format/folder_id/project_id/version/published_at) + the page
> SEARCH index pair; slug helpers lifted to a shared `src/slug.rs` (`slugify`/`unique_slug` now table-generic),
> reused by projects + pages. `pages/` module (model/repo/routes) mirrors `documents/` with the net-new
> slug/visibility/publish logic; body sanitized via `convert::sanitize` on every write; `from_document` promote
> bridge; `kn_link` scrub on `delete_page`; `page` added to the `knowledge` membership whitelists and
> `folder_is_empty`. Owner-scoped REST (`/pages`, `/pages/{id}`, `/publish`, `/unpublish`) behind `CurrentOwner`.
> `tests/pages.rs` (442 lines) covers CRUD/isolation/slug/sanitize/visibility/version/folder/search/promote/membership.

### B — Public serving + visibility/sharing (the deferred public surface)

- [x] **12.5** Public render module `pages/public.rs` + router `GET /p/{slug}`, **wired outside the credentialed CORS layer and the auth-gated tree**. Concretely: restructure `routes/mod.rs` so the public router is built as its own `Router` and merged so the global `.layer(cors)` (currently blanketing every route) and any future Phase 2 auth middleware never apply to it — the serve handler takes **no `CurrentOwner` extractor**. Resolve `slug` to a row with `visibility ∈ {unlisted, public}` (draft/missing → `404`, never confirming existence). Run `convert::harden_for_render` over the stored (already-sanitized) `body` at serve time (strips any remote `<img>` an agent embedded → no viewer-side fetch to internal hosts), wrap it in a minimal self-contained HTML document with an inline stylesheet (export `convert.rs`'s `PRINT_CSS` as `pub`, or duplicate a minimal CSS in `pages/public.rs` — `harden_for_render` cannot run over a full `<!doctype>` document, so the harden pass runs on the **fragment** first, then the fragment is wrapped), and return it with the locked-down header set below. Owner-scoping is intentionally absent on this read — a public page is public — but the *row* still carries its owner for the eventual Phase 2 per-user surface. The serve handler writes **no `activity` row** (the read is anonymous; provenance lives on the authoring tools).
- [x] **12.6** Security headers + origin isolation on `/p/*` (the core of the phase). Response carries: `Content-Type: text/html; charset=utf-8`; a strict CSP — `default-src 'none'; img-src data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; sandbox` (bare `sandbox`, no `allow-*` — no `allow-popups`, so author links can't `window.open` an attacker origin; `img-src data:` only, since `harden_for_render` guarantees no remote/`'self'` image source survives); `X-Content-Type-Options: nosniff`; `Referrer-Policy: no-referrer`; and `X-Robots-Tag: noindex` for `unlisted` (public pages stay indexable). For `unlisted` responses also send `Cache-Control: private, no-store` so an intermediary never retains the secret-URL content. Add `PUBLIC_PAGE_ORIGIN` config (documented in `.env.example`) so `/p/*` can be served from a **distinct origin** from the app/cookies; the publish-time `public_url` builder reads it (falls back to the API origin in single-host dev, where the `sandbox` opaque-origin CSP is the active protection and the separate origin is the documented Phase-2 gate). For `unlisted`, append slug entropy in the slug generator so links aren't enumerable.
- [x] **12.7 (tests-with-features, secure-serving tests)** Extend `tests/pages.rs`: `GET /p/{slug}` returns `200` + the body for public/unlisted and `404` for draft/missing/unknown; **assert every security header is present and exact** (the full CSP string, `nosniff`, `frame-ancestors`/no `allow-*` sandbox, `Referrer-Policy`, `noindex` only for unlisted, `no-store` only for unlisted); assert no `Access-Control-Allow-Credentials`/app-CORS header is emitted on `/p/*`; assert a stored `<script>`/`onerror`/remote-`<img src=http://internal>` never appears in the served bytes (sanitize on write + harden on serve both apply); assert the public GET ignores cookies/owner (a page published by owner A serves with no auth, and *draft* pages 404 regardless). These are the analogue of the Phase 4 secure-download-header tests.

> **Milestone B shipped** (branch `phase-12-web-hosting`, commit `b5ea2f3`). `pages/public.rs` serves
> `GET /p/{slug}` for `visibility ∈ {unlisted, public}` (draft/missing → 404, never confirming existence),
> running `convert::harden_for_render` over the stored body at serve time then wrapping it in a self-contained
> HTML doc. `routes/mod.rs` restructured so the public router is merged **outside** the credentialed CORS layer
> and takes no `CurrentOwner` extractor. Locked-down header set on `/p/*`: the exact `default-src 'none'; …;
> sandbox` CSP, `nosniff`, `no-referrer`, `X-Robots-Tag: noindex` + `Cache-Control: private, no-store` for
> `unlisted`. `PUBLIC_PAGE_ORIGIN` config added (`.env.example`) for the publish-time `public_url` builder
> (falls back to the API origin in single-host dev). `tests/pages.rs` (+108 lines) asserts the full header set,
> serve/404 behaviour, no app-CORS header, and sanitize-on-write + harden-on-serve over the served bytes.

### C — Search/filter/folders + MCP tools + portal

- [x] **12.8** Search & filter: add a `page_ft_title`/`page_ft_body` SEARCH index pair over `page(title, body)` directly in `0010_pages.surql` (the migration runner commits each file separately and the `kn_text` analyzer is already committed by `0007`, so no separate `0011` file is needed — gated on the same 11.1 capability check, falling back to `string::lowercase CONTAINS` if `SEARCH` is unavailable). Extend `knowledge::repo::search` + the `SearchResults` struct with a `pages` typed section (one more `search_two(db, owner, "page", "title", "body", q, limit)` call; `title_col` already returns `title` for `page` with no change), and add `pages` to the REST/MCP `SearchResults` serializer and the frontend `SearchResults` type. Pages then appear in `knowledge_search` / `GET /knowledge/search` and the Phase 11 portal search. The portal search UI must **escape/strip `<mark>`-wrapped HTML snippets** before rendering (page `body` is HTML; an unescaped snippet is stored-XSS in the app origin) — the same treatment documents' snippets need. List filters (12.3) cover folder, visibility, type-already-implied, date (`updated_at` ordering), and author (pre-auth no-op).
- [x] **12.9** MCP tools (mirror the `documents_*` conventions exactly; **owner-scoped via the `owner: &str` arg** of `tools::call`, which takes no `Actor`): `pages_list` (filters: folder, visibility, q), `pages_get`, `pages_create` (body Markdown or HTML via a `source` arg → `convert::md_to_html` then `sanitize`; visibility defaults to **draft** so an agent's page is never self-published to public), `pages_update`, `pages_delete`, `pages_publish` (set visibility, return a result carrying **`id`, `title`, and the public `url`** — `id`+`title` are required so `activity::entry_for` populates `target_id`/`target_title`, mirroring `documents_publish`'s `{published, id, title, …}` shape), `pages_unpublish`. Add `page.create|update|delete|publish|unpublish` arms (target_type `page`) to `activity::entry_for` so each mutating call is logged in `handle_tools_call` to the `agent` label. **In the same commit**, update the `call()` match in `mcp/tools.rs`, the `known` name list, the `assert_eq!(advertised.len(), known.len())` count, and the `def(…)` catalog entries — the drift guard reds otherwise. Enrich the `initialize` instructions with "PUBLISH — author a page and share its URL with pages_publish".
- [x] **12.10 (tests-with-features, mock-LLM, no egress)** MCP tool tests in `tests/pages.rs` / extend `tests/mcp.rs`: `pages_create`→`pages_publish` returns a non-empty `url`; `tools/list` advertises the new tools and the count assert holds; an `activity_list` after a publish shows a `page.publish` row with a non-empty `target_id`/`target_title` attributed to the `X-Baitler-Agent` label; the end-to-end agent script (extend 11.9's) authors a page under a project, publishes it, and `GET /p/{slug}` serves it.
- [x] **12.11** Frontend `PagesPage` (lazy-loaded; add a `navItems` entry in `navigation.ts` (phase 12) **and** a `FEATURE_PAGES` map entry in `App.tsx`, the route-element pattern the other features use): reuse the Phase 7 **TipTap editor + `MarkdownEditor`** for authoring (no new editor; HTML pages instantiate the same `RichTextEditor`) and the Phase 4 **folder/breadcrumb + ConfirmModal** patterns for organisation. A list with filters (folder, visibility chip, `q` search, type/date — the `author` filter shown but disabled-with-tooltip pre-auth), a **publish/visibility toggle** (draft/unlisted/public) with a **copy-the-share-link** clipboard affordance on the `public_url`, a **preview** rendered **only** in a sandboxed `<iframe sandbox src="/p/{slug}">` against the (cross-origin, isolated) serve route — never by injecting page `body` into the app DOM (which would re-open remote-`<img>`/XSS in the app origin) — and the reusable `ExportMenu` for PDF/Word/MD. Add a frontend `pages/api.ts` + `types.ts` mirroring `documents/`. Frontend tests (`vitest`): list/filter render, publish-toggle calls the mutation, copy-link writes the clipboard, the preview iframe has `sandbox`.

> **Milestone C complete** (branch `phase-12-web-hosting`, working tree — verified green, **not yet committed**).
> 12.8: `page` SEARCH index pair lives in `0010_pages.surql` (analyzer `kn_text` already committed by 0007);
> `knowledge::repo::search` + `SearchResults` gain a `pages` typed section, so pages surface in
> `knowledge_search` / `GET /knowledge/search`. (The Phase 11 portal exposes no search UI yet, so the
> frontend `SearchResults` type change is moot until that lands.) 12.9: seven `pages_*` MCP tools
> (`list/get/create/update/delete/publish/unpublish`), `pages_create` defaulting to `visibility="draft"`;
> five `page.*` arms added to `activity::entry_for`; the `call()` match, `known` list, `assert_eq!` count, and
> `def(…)` catalog all moved in lockstep — **45 MCP tools** total. 12.10: `tests/mcp.rs` (+139 lines) covers
> create→publish→`url`, the count assert, the attributed `page.publish` activity row, and the end-to-end
> author→publish→serve script. 12.11: lazy `PagesPage` (TipTap/`MarkdownEditor` authoring, folder/breadcrumb +
> ConfirmModal, publish/visibility toggle + copy-share-link, sandboxed-`<iframe>` preview against `/p/{slug}`,
> `ExportMenu`) + `pages/{api,types}.ts`, wired in `App.tsx` + `navigation.ts`.
> **Verified green: backend `cargo test` (110), frontend `vitest` (43).** Remaining: commit the working tree;
> a live browser screenshot of the authoring/serve flow.

### D — Multi-user / auth-gated sharing (post Phase 2)

- [ ] **12.12** *(after Phase 2)* When auth lands, the `Actor`/`CurrentOwner` swap makes pages per-real-user with **zero page-query changes** (every authoring route is already owner-scoped). Then add, only on real need: per-recipient/per-link sharing (share a page *to* a named user), password-protected pages, the now-meaningful **author filter** (add the `created_by` column then — one `DEFINE FIELD … IF NOT EXISTS`), custom domains, and optional page analytics. The public `GET /p/{slug}` stays unauthenticated by design; auth only gates *authoring/management* and any private-share variant. Keep `/p/*` on its isolated origin, and ensure the Phase 2 auth cookie is **never** set on an origin that serves `/p/*`.

> **Phase 2 dependency (crisp):** Milestones A–C ship **now** against the `DEV_OWNER` stub — a single user can author, organise, publish, and share pages by link, and an agent can do the same over MCP. What needs Phase 2: a *second user* to share *to*, per-user/per-link ACLs, the author filter (one owner today), and any private/password-gated share. The public read surface (`GET /p/{slug}`) is unauthenticated *by nature* and ships in B — auth never gates it; it only gates the management half. The **hard** auth coupling is the cookie-origin tripwire: Phase 2 must not place an auth cookie on any origin that also serves `/p/*`. Sequencing matches the Phase 11 rule — "any public publishing surface AFTER Phase 2 auth" applies to *multi-user* sharing, while the single-owner public link is the minimal slice safe to ship earlier behind strict origin isolation + CSP.

> Acceptance: a `page` authored from Markdown or HTML (portal or MCP) is **sanitized on write** (a stored `<script>` never survives, so even a draft preview is safe), filed in a Phase 4 folder, optionally joined to a project and cross-linked, found by `knowledge_search`, and — once published — served at `GET /p/{slug}` as locked-down HTML from an isolated origin with the exact strict CSP + nosniff + noindex(unlisted), with `harden_for_render` applied at serve time so a remote `<img>` never reaches the served bytes; drafts/missing 404; `pages_publish` (REST or MCP) returns the shareable URL and writes a `page.publish` activity row (with id+title) attributed to the agent; the `/p/*` router is verifiably outside the credentialed CORS layer. The page is a **distinct table from `document`** (creating a page mutates no document) and **reuses the Phase 4 `folder` table** (no second hierarchy). Backend `build`/`clippy -D warnings`/`test`/`fmt` and frontend `tsc`/`eslint`/`vitest`/`build` green; the secure-serving header tests pass; no test needs egress or keys; the `mcp/tools.rs` drift guard is updated in lockstep. **Deferred:** a published-HTML snapshot column, share-token/rotation, versioning/history, CDN/edge cache + ETag, in-process rate limiting (Phase 10), templating/site-builder + a `site` table, per-page asset table/route, per-page analytics, `tags`/`created_by` columns, multi-user/per-link ACLs, password protection, custom domains (all post Phase 2 or on real need).

## Phase 13 — Claude Code CLI integration (in-app agent runner)

Goal: let a user invoke **Claude Code** from inside Baitler to perform real tasks against their own data — "organise my inbox", "draft a README for this project", "answer this from my KB and publish a page" — by wrapping the `claude` CLI as a **server-side, headless, sandboxed** subprocess whose work streams live to the portal and whose writes land in the same owner-scoped knowledge base, attributed and draft-gated like any other agent.

Vision: "Run Claude Code on my Baitler data, from Baitler." Phase 7.5/11 made Baitler an MCP **server** that *external* Claude Code instances drive; this phase closes the loop the other direction — Baitler becomes a **client of the Claude Code CLI**, spawning it non-interactively and pointing it **back at Baitler's own `/mcp`** so the agent reads and writes the user's projects/ideas/documents/pages with the tools the rest of the system already exposes. The CLI wrapper the brief asks for is a thin internal seam (`cli/`): the application builds a constrained `claude -p … --output-format stream-json` invocation, never a shell string, and surfaces the streamed transcript and result to the user — no terminal, no raw flags, no host shell. The agentic story gains a *self-service* front door: instead of the user wiring Claude Code to Baitler by hand (per `docs/mcp.md`), they type a task in the portal and Baitler orchestrates the run.

Architecture: everything is additive. A new `cli/` module defines an `AgentRunner` trait — **mirroring `llm/provider.rs` verbatim** — with two implementations: a **`MockRunner`** (deterministic scripted `RunEvent`s, no binary, no egress — the CI-tested path, exactly as `llm/mock.rs` is the asserted LLM path) and a **`ClaudeCliRunner`** that spawns `CLAUDE_BIN` via `tokio::process::Command`, streams the child's `stdout`, and parses Claude Code's `--output-format stream-json` JSONL into a typed `RunEvent` enum (`Init`/`Assistant`/`ToolUse`/`ToolResult`/`Result`/`Error`). A `cli_run` table (migration `0011_cli_runs.surql`) records each invocation; runs stream over **SSE reusing the Phase 6 `ai/routes.rs` pattern**; each run gets an **ephemeral per-run sandbox working directory** and a **generated `--mcp-config`** that loops back to Baitler's `/mcp` with `X-Baitler-Agent: claude-code`. The owner's Anthropic key comes from the **Phase 6 encrypted `provider_key` store** (`crypto.rs`), injected into the **child env only**. No existing table or module is rewritten; the only edits to existing code are router wiring, two `activity::entry_for` arms (`cli.run`), and an `.env.example` block. **Decisions taken up front:** (a) **Reuse, don't reinvent** — the runner trait copies the `LlmProvider` Mock/real split, SSE copies `ai/routes.rs`, the key comes from the Phase 6 store, results save through the existing idea/document/page repos, and MCP writes inherit the Phase 11 `review="draft"` gate + `activity` attribution. (b) **MCP loopback is the whole point** — the spawned Claude Code is configured with `--strict-mcp-config --mcp-config <generated>` aimed at Baitler's own server, so it operates on the user's KB rather than the host filesystem; host tools (`Bash`/`Write`/`Edit`) are **disallowed by default**. (c) **Headless only** — `claude -p` with `--output-format stream-json --verbose`; no interactive TTY, and the Baitler MCP tools are **pre-authorised via `--allowedTools`** so no permission prompt can block a non-interactive run (never `--dangerously-skip-permissions`). (d) A purpose-built **`cli_run` table, not a generic job queue** — a subprocess agent runs for *minutes* and outlives a quick request, and the user wants history + resume, which is exactly the case Phase 11 declined a jobs layer for (that was one bounded 30s Chrome render; this is a long, interactive, resumable agent). (e) **Off by default** (`CLAUDE_CLI_ENABLED=false`) and **egress-required**, so — like the real LLM adapters — the binary is absent and unexercised here; the `MockRunner` is the end-to-end-tested path and a missing/disabled binary returns a clear **503** (mirroring Pandoc-absent in Phase 7).

### Security reality (read before A) — this is the whole risk of the phase

- **Spawning an autonomous coding agent as a subprocess is the headline risk** — strictly larger than Phase 12's "serve user HTML". Claude Code can call tools, and with host tools it can run bash and edit files. The feature is therefore **opt-in and off by default** (`CLAUDE_CLI_ENABLED=false`); when enabled, the default tool scope is **Baitler MCP tools only** (`--allowedTools mcp__baitler__*`), host `Bash`/`Write`/`Edit` are **disallowed** unless `CLAUDE_CLI_ALLOW_HOST_TOOLS=true`, and `--dangerously-skip-permissions` is **never** passed.
- **No shell, ever — argv is built as a vector.** The user supplies a *prompt* plus a *constrained set of options* (model, target project, max-turns, tool-scope toggle); Baitler constructs the `claude` argv as a `Vec<String>` and passes the prompt as a single argv element (or over stdin via `--input-format`), so there is no shell string to inject into. **Raw CLI flags are never accepted from the client** — the wrapper owns the flag set.
- **Secrets go in the child env, never argv.** The Anthropic key is pulled from the Phase 6 encrypted store and set as `ANTHROPIC_API_KEY` on the child process env (argv is world-readable via `ps`); it is never logged, never echoed in a `RunEvent`, and never written to the `cli_run` row.
- **Filesystem isolation: an ephemeral per-run sandbox cwd.** Each run executes in a throwaway directory under `CLAUDE_CLI_WORKDIR`, created before and removed after; `--add-dir` is limited to that sandbox. The Baitler repo, the SurrealDB data dir, and the rest of the host filesystem are **never** exposed to the child. (Hardened OS-level isolation — container/cgroup/seccomp, network egress allowlist — is a Phase D / Phase 10 upgrade; sandbox-dir + tool-deny + timeout is the right slice for single-user dev.)
- **Loopback writes are draft-gated and attributed — reuse, don't re-secure.** Because the spawned agent reaches the KB only through Baitler's `/mcp` with `X-Baitler-Agent: claude-code`, its writes land as `review="draft"` (Phase 11 gate) and every mutation logs an `activity` row — so it **can't silently publish**, and the human approves in the existing Review queue. `--strict-mcp-config` ensures no ambient/global MCP servers from the host leak into the run.
- **Resource exhaustion on a single-user host.** Each run enforces a **timeout** (kills the whole process group, not just the parent), a `--max-turns` cap, and an output-size cap; runs are **serialised per owner** (one active run at a time) so a runaway agent can't fork-bomb the box. `POST /cli/runs/{id}/cancel` kills the process group and marks the row `cancelled`.
- **Egress reality (state it like Phase 6).** A real run needs network egress to the Anthropic API and a valid key; **this env has neither**, so the `MockRunner` is the asserted path and no test spawns the real binary or hits the network. The real `ClaudeCliRunner` is compiled and wired but unexercised in CI.
- **Recursion guard.** A `cli_run` MCP **tool** (an external agent triggering a Claude Code run) is an agent-spawns-agent loop and is **deferred** behind an explicit second flag + a depth guard (13.10) — the headline use case is a *human in the portal* invoking a run, not an agent recursing.
- **Over-engineering to avoid (personal app):** a distributed job queue / worker pool, a websocket multiplexer (SSE suffices, as Phase 6 proved), a full container orchestrator for A–C, a structured per-event transcript table (stream over SSE; persist the final result + a derived summary, not every tool body verbatim), arbitrary raw-flag passthrough, multi-tenant cost/concurrency quotas (one owner today — a single per-owner serialise is enough), and a bespoke permission-prompt MCP tool (pre-authorised `--allowedTools` removes the prompt entirely for the headless path).

### A — CLI wrapper + run model + streamed run (Baitler-MCP tools only, no host shell)

- [x] **13.1** New `cli/` module (`mod.rs,runner.rs,events.rs,repo.rs,routes.rs`) defining the `AgentRunner` trait (`async fn run(&self, spec: RunSpec) -> Result<EventStream>`) **modelled on `llm/provider.rs`**, a typed `RunEvent` enum (`Init{session_id,model}`, `Assistant{text}`, `ToolUse{name,summary}`, `ToolResult{ok,summary}`, `Result{text,session_id,num_turns,cost_usd,is_error}`, `Error{message}`), and a `RunSpec` (owner, prompt, model, project_id, max_turns, tool_scope ∈ {kb_only, kb_plus_read}, optional `resume_session_id`). Ship **`MockRunner`** first (deterministic scripted events: init → assistant → one `mcp__baitler__ideas_create` tool-use/result → result — the CI path, mirroring `llm/mock.rs`). Config in `config.rs`: `CLAUDE_CLI_ENABLED` (default **false**), `CLAUDE_BIN` (default `claude`), `CLAUDE_CLI_WORKDIR`, `CLAUDE_CLI_TIMEOUT_SECS` (default 600), `CLAUDE_CLI_MAX_TURNS` (default 24), `CLAUDE_CLI_DEFAULT_MODEL`, `CLAUDE_CLI_ALLOW_HOST_TOOLS` (default **false**) — fail-fast validated like the rest of `Config`.
- [x] **13.2** `ClaudeCliRunner`: spawn `CLAUDE_BIN` via `tokio::process::Command` with a **server-built argv vector** — `-p`, `--output-format stream-json`, `--verbose`, `--model <m>`, `--max-turns <n>`, `--permission-mode default`, `--strict-mcp-config`, `--mcp-config <generated path>` (13.3), `--allowedTools mcp__baitler__*` (plus `Read` only when `tool_scope=kb_plus_read`), `--disallowedTools Bash Write Edit` unless `CLAUDE_CLI_ALLOW_HOST_TOOLS`, `--add-dir <sandbox>`, and `--resume <id>` when resuming. **Never** `--dangerously-skip-permissions`. Set `ANTHROPIC_API_KEY` (from the Phase 6 store) on the **child env only**; cwd = a freshly created per-run sandbox dir under `CLAUDE_CLI_WORKDIR` (removed on completion). Read `stdout` line-by-line, parse each JSONL line into a `RunEvent` (tolerating unknown event types — forward as a generic `Assistant`/ignore, never panic), enforce the timeout by killing the **process group**, and cap accumulated output. Prompt passed as the `-p` argv element (or stdin) — never interpolated into a shell.
- [x] **13.3** Per-run MCP loopback config: generate a temporary `--mcp-config` JSON declaring a single `baitler` HTTP server pointing at this process's own `/mcp` URL, carrying the optional `MCP_AUTH_TOKEN` bearer and the `X-Baitler-Agent: claude-code` header so every loopback write is attributed in `activity_list` and draft-gated (Phase 11). `--strict-mcp-config` guarantees **only** this server loads (no host/global MCP servers leak in). The config is written into the run sandbox and torn down with it; the key/token are file-scoped to the sandbox (0600), never in argv.
- [x] **13.4** Migration `0011_cli_runs.surql` (SCHEMAFULL): `cli_run` — `uuid` (UNIQUE `cli_run_uuid`), `owner`, `prompt` (the user's own text), `model`, `tool_scope`, `status` DEFAULT "queued" (queued|running|succeeded|failed|cancelled), `session_id` `option<string>`, `num_turns` int DEFAULT 0, `cost_usd` `option<float>`, `exit_code` `option<int>`, `result_text` `option<string>`, `error` `option<string>`, `project_id` `option<string>` (Phase 11 membership pointer), `created_at`/`updated_at`/`finished_at` `option<datetime>`. Index `cli_run_owner` ON owner,created_at. Store a **result + derived summary**, never verbatim tool bodies.
- [x] **13.5** Owner-scoped REST in `cli/routes.rs` (behind `CurrentOwner`, alongside `/ai`): `POST /cli/runs` creates the row and **streams `RunEvent`s over SSE** (reuse the `ai/routes.rs` `Sse`/`KeepAlive` pattern), persisting `running`→terminal status + result/cost/session on completion; `GET /cli/runs` (paginated, owner-scoped, optional `project`/`status` filter), `GET /cli/runs/{id}`, `POST /cli/runs/{id}/cancel` (kills the process group, sets `cancelled`). Return **503** when `CLAUDE_CLI_ENABLED=false`, the binary is absent, or no Anthropic key is configured for the owner (clear message, mirroring Pandoc-absent 503). Serialise to **one active run per owner** (409 if one is already running).
- [x] **13.6 (tests-with-features, Mock runner, no egress)** `tests/cli.rs`: full run lifecycle against `MockRunner` (queued→running→succeeded, row persisted, SSE event sequence shape asserted); **argv-builder unit asserts** — no `--dangerously-skip-permissions`, `Bash`/`Write`/`Edit` in `--disallowedTools` by default, `--strict-mcp-config` present, the prompt is a discrete argv element, and **the API key never appears in argv**; sandbox dir created then removed; owner isolation (two synthetic owners, a run is invisible to the other); cancel transitions to `cancelled`; **503 when disabled** and **409 on a second concurrent run**.

> **Milestone A shipped** (branch `phase-12-web-hosting`, working tree — verified green, **not yet committed**).
> New `cli/` module: `runner.rs` (`AgentRunner` trait + `RunSpec`/`ToolScope`/`RunError`, modelled on `llm/provider.rs`),
> `events.rs` (`RunEvent`), `mock.rs` (`MockRunner` — the CI path), `claude.rs` (`ClaudeCliRunner` + pure
> `build_argv`/`build_mcp_config`/`parse_line` helpers), `registry.rs` (`RunRegistry` — per-owner serialisation +
> cancellation), `model.rs`/`repo.rs`/`routes.rs`. Migration `0011_cli_runs.surql`. `CliConfig` added to `config.rs`
> (fail-fast `from_env`, `Default` = disabled + `bin="mock"`); `AppState` gains `cli_runner` + `cli_runs`. As-built
> deltas from the design: the runner is selected by the `CLAUDE_BIN == "mock"` sentinel (mirrors `SURREAL_URL=memory`)
> rather than a separate test knob; the row starts `running` (no `queued` state — runs stream synchronously);
> `--allowedTools mcp__baitler` (server-level approval) not the `mcp__baitler__*` wildcard; the timeout/cancel kill the
> child via `kill_on_drop` + `start_kill` (true process-**group** kill is folded into the Phase 13.D OS-isolation
> upgrade — the loopback uses HTTP MCP, so `claude` spawns no stdio MCP subprocess). Activity logging (`cli.run`) is
> deferred to 13.9 with the frontend. **Verified green: backend `cargo test` (122 — +12), `clippy -D warnings`, `fmt`.**
> Env documented in `.env.example`. Remaining: commit; Milestones B/C (frontend + resume/provenance).

### B — Frontend agent console

- [x] **13.7** Lazy `AgentPage` (add a `navItems` entry in `navigation.ts` (phase 13) **and** a `FEATURE_*` map entry in `App.tsx`, the route-element pattern the other features use): a **task composer** (prompt textarea, model picker reusing `GET /ai/providers`, optional target-project select, tool-scope toggle [KB-only / +Read], max-turns) with a Run button; a **live streamed transcript** reusing the Phase 6 SSE parser — assistant text Markdown-rendered (existing `react-markdown`), tool-use/tool-result rendered as compact chips, with a Stop button wired to `cancel`; a footer showing session id / turns / cost when the `Result` event lands. A disabled state with the 503 reason when the runner is off. Frontend `cli/api.ts` + `types.ts` mirroring `ai/`.
- [x] **13.8 (tests-with-features)** Run **history** list + detail (re-open a past run, see its result and metadata) and a **"Save result as…"** affordance (Idea / Document / Page) calling the existing creators so the artifact lands as a **draft** (Phase 11 gate) — no new persistence code. `vitest`: composer submits and renders streamed events, Stop calls the cancel mutation, the disabled/503 state renders, Save-as calls the right creator with `review="draft"`.

> **Milestone B shipped** (branch `phase-12-web-hosting`, working tree — verified green, **not yet committed**).
> Lazy `AgentPage` (`frontend/src/features/cli/`): `types.ts` (`RunEvent`/`CliRun`/…), `api.ts` (`streamRun` SSE
> parser mirroring `ai/streamChat`, `useRuns`/`useRun`/`useCancelRun`/`useProjectOptions`, `saveResultAs`), `AgentPage.tsx`
> (composer — prompt + Anthropic-model select from `/ai/providers` + project select + tool-scope + max-turns; live
> transcript with Markdown assistant text + tool-use/tool-result chips + a session/turns/cost footer; Stop wired to the
> cancel endpoint; a 503 disabled banner; a **Recent runs** history sidebar that re-opens a past run's detail; a
> **Save result as** draft Idea/Document/Page affordance). Wired in `App.tsx` (`/agent`) + `navigation.ts` (phase 13,
> `Bot` icon). One small backend tweak: `run_sse` now emits a leading `{type:"run",id}` SSE event so the client can
> cancel the in-flight run (clean DB `cancelled` status vs. just dropping the socket). As-built: the model picker lists
> Anthropic models (the CLI runs Claude); save-as creates then PATCHes `review="draft"` for ideas/documents (pages
> default to draft). **Verified green: backend `cargo test` (122), frontend `tsc`/`eslint`/`vitest` (49 — +6)/`vite build`
> (`AgentPage` code-split).** Remaining: commit; Milestone C (resume/project-scoping/`cli.run` activity arm).
>
> **Post-B fixes (working tree):** (1) the stored Anthropic key is now **optional** — `uses_key` (was `requires_key`)
> only injects a Baitler-stored key into the child env *when present*; with none, the child inherits host auth (a
> `claude login` session in `$HOME/.claude`, or a host `ANTHROPIC_API_KEY`), so the endpoint never 503s for a missing
> Baitler key (an auth failure surfaces as a run error event instead). (2) **Preflight** — `AgentRunner::health()`
> probes `claude --version`; `GET /cli/status` composes `enabled`/`kind`/`binary_ok`/`version`/key-signals into a
> `ready` flag + human hint; `cli::log_startup` logs one readiness line at boot; the Agent page's `useCliStatus` shows
> a blocking banner (disables Run) when not ready and a soft auth hint when ready-but-no-key-detected. **Verified green:
> backend 123 tests + clippy/fmt; frontend 50 tests + tsc/eslint/build.**
> (3) **Sandbox path bug fix** — an empty `CLAUDE_CLI_WORKDIR`/`CLAUDE_BIN` (blank line from `.env.example`) now falls
> back to its default (was becoming `""`), and the per-run sandbox is canonicalized to an absolute path so the child's
> relative `--mcp-config`/`--add-dir` can't double-resolve against its cwd. (4) **stderr capture** — the child's stderr
> is drained (capped) and, on a no-result exit, the exit code + stderr tail are surfaced in the error event (was a bare
> "exited without a result").

> **MiniMax agent provider added (working tree).** A second selectable agent backend that drives the **same `claude`
> CLI** against MiniMax's Anthropic-compatible endpoint (`https://api.minimax.io/anthropic`, key `ANTHROPIC_AUTH_TOKEN`,
> model `MiniMax-M3` — per MiniMax docs). Backend: `CliConfig` gains `minimax_api_key`/`base_url`/`model` (from
> `MINIMAX_API_KEY`/`MINIMAX_BASE_URL`/`MINIMAX_MODEL`; key redacted in `Debug`); `AgentProvider` enum + `RunSpec.provider`;
> a pure, unit-tested `provider_env` injects `ANTHROPIC_BASE_URL`+`ANTHROPIC_AUTH_TOKEN` for MiniMax (vs the optional
> `ANTHROPIC_API_KEY` for Claude Code); the route parses `provider`, resolves the per-provider key (MiniMax requires
> `MINIMAX_API_KEY` → clear 503) + model default, and `GET /cli/status` returns a `providers[]` array with per-provider
> availability + detail. Frontend: an **Agent** selector in the composer (Claude Code / MiniMax-M3) populated from
> `status.providers`; model options switch by provider; Run is blocked with the provider's detail when the selected one
> is unavailable. **Verified green: backend 126 tests + clippy/fmt; frontend 51 tests + tsc/eslint/build.** Both providers
> still require `CLAUDE_CLI_ENABLED=true` + an installed `claude`.

> **Agent dock — right-hand pane on every feature page (working tree).** The agent console is now extracted into a
> reusable `AgentPanel` (single-column, scrollable, `embedded` prop) and docked as a right-hand pane on every feature
> page so the agent can act on what the user is viewing. Frontend: `AgentPanel.tsx` (panel logic moved from `AgentPage`,
> which is now a thin wrapper for the `/agent` route); `AgentDock.tsx` (fixed slide-over <lg, side column ≥lg via
> `lg:pr-96` in `AppLayout`); `stores/agentDock.ts` (persisted open/closed); a `Bot` toggle in the header (hidden on
> `/agent`); `context.ts::pageContext` maps the route → a page label + an orienting context string. The dock passes that
> **`context`** with each run → backend appends it to the system prompt via **`--append-system-prompt`** (new `context`
> field on `RunSpec`/`CreateRunBody`, capped at 4k, unit-tested in `build_argv`). The agent still reaches all data
> through the MCP loopback; the context just orients it to the current page. **Verified green: backend 127 tests +
> clippy/fmt; frontend 53 tests + tsc/eslint/build.**

> **Resizable dock + local-folder grant (working tree).** (1) The dock is now **width-resizable**: a left-edge drag
> handle (+ arrow-key resize), width persisted in the `agentDock` store (clamped 320–960px), and `AppLayout` reserves
> matching space on `lg+` via a `--agent-dock-w` CSS var. (2) **Local-folder import** — the agent can be granted
> **read-only** access to an allow-listed host folder so it can import disk files into Baitler. Backend:
> `CliConfig.workspace_roots` (`CLAUDE_CLI_WORKSPACE_ROOTS`, colon/comma-sep absolute dirs; empty = off);
> `cli::validate_workspace` canonicalizes the requested folder and verifies (component-wise `starts_with`) it's inside a
> root — defeating `..`/symlink/look-alike escapes; `RunSpec.workspace_dir` → `build_argv` adds `--add-dir <dir>` and
> pre-approves read-only `Read`/`Glob`/`Grep` (host `Bash`/`Write`/`Edit` stay denied); the route validates
> `workspace_dir` (400 if outside roots / not enabled) and `/cli/status` reports `workspace_roots`. Frontend: a **Local
> folder** input in the composer, shown only when roots are configured, with the allowed-roots hint. **Verified green:
> backend 130 tests + clippy/fmt; frontend 54 tests + tsc/eslint/build.** This is the mechanism for the "import my
> local Pictures" request: set `CLAUDE_CLI_WORKSPACE_ROOTS=/home/bart`, then point a run at `/home/bart/Pictures`.

> **`files_import` MCP tool — server-side local import (working tree).** Discovered gap: a sandboxed agent can *see*
> local images via `Read` but can't get their raw **bytes** to base64 into `files_write`, and it has no shell. Fix: a new
> **`files_import`** MCP tool (the **46th**) that takes an absolute local file/dir path and imports it into Baitler Files
> **server-side** — Baitler reads the bytes from disk directly (no base64 through the agent's context, no shell). Gated by
> the same `CLAUDE_CLI_WORKSPACE_ROOTS` allow-list (`cli::resolve_under_roots`, canonicalized, within-root, symlinks not
> followed); a directory imports its files (optional recursion, ≤500/call), per-file MIME by extension, ≤`MAX_BLOB` each,
> storage-rollback on metadata failure, attributed via a `file.import` activity arm. Drift guard moved in lockstep
> (dispatch + `def` catalog + `known` list + count = 46). **Verified green: backend 131 tests + clippy/fmt.** The agent
> can now do: `files_import({ path: "/home/bart/Pictures", recursive: false })` → images land in Files. CLAUDE.md tool
> count bumped 43→46.

> **Agent panel is now a chat conversation (working tree).** `AgentPanel` was reworked from one-shot runs into a
> multi-turn **chat thread**: each message continues the same agent session by sending `resume_session_id` (the
> `session_id` captured from the prior run's `init`/`result` event → `--resume`, which the backend already supported).
> The composer sits at the bottom (Enter-to-send, Shift+Enter newline), user/agent turns stack in a scrolling thread
> (assistant Markdown + tool chips inline), the Run button reads **Send** once a session exists, a **New chat** button
> resets, and a **Previous chats** picker re-opens a past run (seeds the thread from its result + resumes its session so
> you can keep talking). Pure-frontend change — no backend edits. **Verified green: frontend 55 tests** (incl. a
> continuity test asserting the 2nd message carries `resume_session_id`) **+ tsc/eslint/build.**

> **Chat resume fix — stable per-conversation working dir (working tree).** Bug: a follow-up message did nothing
> ("Done · 0 turns · $0.00") because `claude` keys resumable sessions to the **cwd**, but every run used a fresh
> throwaway sandbox — so `--resume <session_id>` found no session. Fix: a client-stable **`conversation_id`** (reused
> across a chat's turns; new on "New chat") maps to a **persistent `conv-<id>`** working dir that's kept between turns
> (one-shot runs still use a throwaway `run-<id>` dir, deleted as before). The id is **sanitized** server-side (ASCII
> alnum + `-`/`_`, ≤64) since it forms a directory name, and **persisted** on the `cli_run` row (migration `0012`) so
> "Previous chats" truly resumes (its `conversation_id` + `session_id` are reused). Threaded through
> `CreateRunBody`/`RunSpec`/`repo::create_run`/`build_argv` (`conv_dirname`) + the row DTOs. **Verified green: backend
> 133 tests + clippy/fmt; frontend 55 tests + tsc/eslint/build.**

> **MiniMax tool/MCP fix — map every model slot (working tree).** MiniMax runs couldn't use MCP tools because Claude
> Code makes background/"small-fast" calls with a *Claude* model name (e.g. `claude-…-haiku`) the MiniMax gateway lacks
> → 404s that stall tool use (the loopback config is identical to Claude Code's and works). Fix: `provider_env` now sets
> `ANTHROPIC_MODEL` + `ANTHROPIC_SMALL_FAST_MODEL` + `ANTHROPIC_DEFAULT_{HAIKU,SONNET,OPUS}_MODEL` all to `MINIMAX_MODEL`
> for the MiniMax provider, so every request stays on the gateway. **Verified green: backend 133 tests + clippy/fmt**
> (the `provider_env` unit test asserts the model-slot mapping).

### C — Resume, project scoping, provenance (and a deferred MCP trigger)

- [ ] **13.9** **Resume / multi-turn**: a follow-up run reuses the prior run's `session_id` via `--resume`, so a conversation continues without re-establishing context. **Project scoping**: a run carries `project_id` so its loopback MCP writes inherit it (the agent's `ideas_create`/`pages_create` land in the chosen project) and the run row is filterable by project in the portal. Add `cli.run` (target_type `cli_run`) to `activity::entry_for` so each completed run is logged to the `claude-code` agent label alongside the loopback writes it produced.
- [ ] **13.10 (deferred / guarded)** Optionally expose a `cli_run` **MCP tool** (so an external agent can trigger a Baitler-orchestrated Claude Code run) behind a **second explicit flag** (`CLAUDE_CLI_MCP_ENABLED`, default false) **and a recursion-depth guard** that refuses to start a run when the caller is itself the `claude-code` agent. If shipped, update the `call()` match + `known` list + `assert_eq!` count + `def(…)` catalog in `mcp/tools.rs` **in the same commit** (drift guard). Default posture: **not exposed** — the human-in-the-portal path (A/B) is the supported one.

### D — Multi-user + hardened isolation (post Phase 2)

- [ ] **13.11** *(after Phase 2)* The `Actor`/`CurrentOwner` swap makes runs per-real-user with **zero query changes** (every route is already owner-scoped). Then add, on real need: **per-user concurrency + cost/turn quotas** (meaningful only with >1 user), **OS-level subprocess isolation** (run the child in a container/cgroup with seccomp and a network egress allowlist limited to the Anthropic API + the loopback `/mcp`), optional **host-tool runs inside a throwaway container** (so `Bash`/`Write`/`Edit` can be enabled safely), and a choice between **Baitler-managed vs user-supplied** Anthropic auth. Keep the run sandbox per-user; never let one user's run read another's sandbox or key.

> **Phase dependencies (crisp):** Milestones A–C ship **now** against the `DEV_OWNER` stub — a single user authors and runs Claude Code tasks against their own KB, watches the stream, and saves results as drafts. What needs **Phase 2**: per-real-user runs, quotas, and the managed-vs-BYO key choice (13.D). What needs a real **egress + a `claude` binary + an Anthropic key**: any non-Mock run — absent here, so the `MockRunner` is the asserted path exactly as the real LLM adapters are unexercised in Phase 6. Builds directly on **Phase 6** (encrypted key store, SSE), **Phase 7.5/11** (the `/mcp` loopback target, the `activity` log + `review="draft"` gate + `X-Baitler-Agent` attribution), and reuses **Phase 11/12** repos for "save result as" — so it can begin once Phase 11 has merged.
>
> Acceptance: with `CLAUDE_CLI_ENABLED=true`, a user issues a task in the portal; Baitler spawns `claude -p … --output-format stream-json --strict-mcp-config --mcp-config <loopback>` in a per-run sandbox with the Anthropic key in the child env (never argv), host tools disallowed and `--dangerously-skip-permissions` never set; `RunEvent`s stream over SSE to the console; the agent's loopback writes land as `review="draft"` in the Review queue, attributed to `claude-code` in `activity_list`; the `cli_run` row records status/session/turns/cost and is resumable; cancel kills the process group; a second concurrent run for the owner is rejected; and with the feature disabled (the default, and this env) the endpoint returns a clear 503 and the `MockRunner` carries the asserted tests. Backend `build`/`clippy -D warnings`/`test`/`fmt` and frontend `tsc`/`eslint`/`vitest`/`build` green; no test needs egress, a key, or the real binary; if the `cli_run` MCP tool is ever shipped (13.10), the `mcp/tools.rs` drift guard moves in lockstep. **Deferred:** the MCP `cli_run` trigger (guarded), OS-level container/seccomp isolation + egress allowlist, host-tool runs, per-user quotas, managed-vs-BYO Anthropic auth, a structured per-event transcript table, and any websocket/job-queue layer (all post Phase 2 or on real need).

## Phase 14 — Knowledge organization & visual modeling (tags, mindmaps, draw.io)

Goal: make stored knowledge easier to organize, find, and reason about — cross-type **tags** on documents/ideas/pages (with the list/search/filter surfaces that use them), a **mindmap** content type for arranging ideas/information visually, and **draw.io** diagram authoring/management — all **additive**, reusing the Phase 4 `folder` tree, the Phase 11 projects/`kn_link`/`knowledge_search`/`review`/`activity` machinery, the MCP `Actor`/drift-guard conventions, and the Phase 7 `convert.rs` export pathway.

Vision: "Tag, map, and diagram my knowledge." Today ideas carry `tags[]` but documents and pages don't, search is text-only, and there's no visual way to relate items. This phase (1) makes tags a **shared vocabulary** across the prose types and threads them through search/filter; (2) adds **mindmaps** — a node/edge graph you author freehand, generate from a Markdown outline, or **seed from a project's ideas + cross-links** — whose nodes can link back to Baitler items so the map is a navigable overlay on the knowledge base; and (3) embeds **draw.io** for real diagrams, stored as portable mxGraph XML with a rendered preview. The agent (Phase 13) gets new MCP tools for all three, so "tag these, mind-map this project, draw the architecture" all work over `/mcp` with the same draft-review gate.

Architecture: everything is purely additive — no existing table is rewritten. **Decisions taken up front:** (a) **Tags are one normalized vocabulary** lifted into `src/tags.rs` (mirroring `src/slug.rs`): `document` and `page` gain a `tags array<string>` column (idea already has one since Phase 5), and the existing `ideas::clean_tags` is generalized and re-pointed at the shared normalizer — this **reverses Phase 12's "no `tags` column" deferral**, now that there's real cross-type need. (b) **Mindmaps and diagrams are new first-class content types** (`mindmap`, `diagram` tables) that **mirror `document`/`page`** (owner/title/folder_id/project_id/tags/review/version/timestamps + a typed body) rather than introducing new graph schemas — the visual structure lives **inside a JSON/XML body**, exactly like a document's HTML body — so they inherit folders, projects, `kn_link`, `knowledge_search`, the draft gate, activity, export, and the MCP/drift-guard conventions with zero new cross-cutting machinery (mirrors the early-ideas decision to model links as a uuid array, not graph edges). (c) **Mindmap canonical model = a JSON node/edge graph**, authored with a client graph editor (**`@xyflow/react`** / React Flow), seedable from a Markdown outline (headings/bullets → tree) or from a project (ideas → nodes, `kn_link`s → edges); nodes may carry an `item_type`/`item_id` pointer so the map navigates to the underlying item. (d) **Diagram canonical model = draw.io (mxGraph) XML**, authored via the **embedded draw.io editor** (an `<iframe>` in `?embed=1&proto=json` mode talking `postMessage`), with a rendered PNG/SVG preview that round-trips (draw.io's PNG-with-embedded-XML). (e) **draw.io runs from a configurable origin** (`DRAWIO_URL`, default the hosted embed; **self-hosting is the privacy-safe option and documented**) and the live editor frame loads **only** on the authoring surface — everywhere else, and on the public `/p/*` page surface, diagrams render as **static sanitized SVG/PNG**. (f) Owner-scoped throughout; agent writes default to `review="draft"`; each new searchable type folds a **typed section into `knowledge_search`** exactly as pages did in 12.8.

### Security reality (read before C) — the draw.io frame is the headline risk

- **draw.io embeds a third-party iframe.** In embed mode the diagram XML stays **client-side** over `postMessage` (draw.io doesn't exfiltrate it), but the iframe loads third-party JS. The editor page's CSP must `frame-src` the **`DRAWIO_URL` origin only**, and **self-hosting** (point `DRAWIO_URL` at an internal deploy) removes the third-party dependency entirely. The live editor is gated to the authoring surface; **diagrams render as static sanitized SVG/PNG everywhere else**, and the public `/p/*` surface **never** embeds draw.io or its frame.
- **Stored XML/JSON/SVG is attacker-authorable** (an agent can author a draft diagram/mindmap before a human looks). A draw.io XML → SVG render is run through **`convert::sanitize`** (ammonia — scripts/event-handlers/`foreignObject` stripped) before any preview/export, mirroring the document/page sanitize-on-write invariant; **mindmap node labels are treated as plain text** (escaped on render, never raw HTML). Any server-side render (PDF) reuses `convert::harden_for_render` + the Phase 11.10 DNS-blocked/sandboxed Chrome.
- **Tags are short opaque strings** — normalized on write (trim, lowercase, slug-ish charset, dedupe, capped count/length) so a tag can't smuggle markup into a search snippet, and escaped wherever rendered (the same treatment snippets already need).
- **No new anonymous/egress surface.** The draw.io editor is a client iframe; mindmap rendering is client-side; the only server-side render is the existing owner-triggered Chrome export. Nothing here is reachable unauthenticated.

### A — Cross-type tags / metadata

> **Milestone A shipped** (working tree — verified green, **not yet committed**). Shared `src/tags.rs::normalize_tags`
> (trim/dedupe/cap — a faithful lift of `ideas::clean_tags`, which now calls it); migration `0013_tags.surql` adds
> `tags array<string>` + indexes to `document` + `page`. Tags threaded through `documents`/`pages` repos (CREATE/UPDATE +
> `*_SELECT` + DTOs/summaries), REST create/PATCH, and the MCP `documents_/pages_ create/update` schemas (tags-only edits
> don't bump `version`). `document`/`page` list endpoints + MCP `*_list` accept a `tag` filter; `GET /tags` + a new MCP
> **`knowledge_tags`** tool aggregate the cross-type taxonomy (idea+document+page, with counts) — **47 MCP tools** now,
> drift guard moved in lockstep. Frontend: the Phase 5 `TagInput` is reused in the Documents + Pages editors (tags
> autosave; don't bump version), `tags` added to the TS types + create/update payloads. **As-built deltas:** the
> normalizer keeps author casing (no lowercasing — a pure lift, zero ideas-behaviour change); the **`knowledge_search`
> tags-on-hits + in-search `tag` filter is deferred** (it'd touch all 5 search variants where `file`/`project` lack a
> `tags` column — fragile; browse-by-tag + the taxonomy already deliver the organize/find win), and a standalone
> tag-filter *UI control* + list tag-chips are deferred (the editors + taxonomy ship now). **Verified green: backend 136
> tests + clippy/fmt; frontend 55 tests + tsc/eslint/build.** CLAUDE.md tool count 46→47.

- [ ] **14.1** Lift a shared tag normalizer into `src/tags.rs` (`normalize_tags`: trim → lowercase → restrict to a slug-ish charset → dedupe → cap count + per-tag length), generalizing `ideas::routes::clean_tags` and **re-pointing ideas at it** (a real refactor owned here, like the Phase 12 slug lift). Migration `0013_tags.surql` (SCHEMAFULL, `DEFINE FIELD IF NOT EXISTS`): add `tags array<string> DEFAULT []` to `document` and `page`, plus `document_tags`/`page_tags` indexes for tag-filtered browse. (`idea.tags` already exists.)
- [ ] **14.2** Thread tags through `documents/repo.rs` + `pages/repo.rs` (the `CREATE CONTENT` + `UPDATE SET` builders, the `*_SELECT` projections, and `DocumentDto`/`PageDto`/summaries), the REST create/PATCH bodies, and the MCP `documents_{create,update}` + `pages_{create,update}` schemas (optional `tags` arg). Normalize on every write via `tags::normalize_tags`; a tags-only edit does **not** bump `version` (mirror the review-only rule).
- [ ] **14.3** Filter + taxonomy + search: documents/pages list endpoints accept a `tag` filter (array `CONTAINS`, as `ideas_repo::list_ideas` already does); `knowledge::repo::search` ANDs an optional `tag` filter with the text query and returns `tags` on each hit (extend `SearchResults` + the REST/MCP/frontend types); a unified **`GET /tags`** taxonomy (distinct tags + per-type counts across idea/document/page) and an MCP **`knowledge_tags`** tool. Tag strings are escaped wherever rendered.
- [ ] **14.4 (tests-with-features)** Frontend: reuse the Phase 5 `TagInput` in the Documents + Pages editors; render clickable tag **chips** on cards/lists (click → applies the `tag` filter); a tag filter control in each feature list. Backend tests (tags round-trip + normalize + filter + `GET /tags` + search-by-tag, owner-scoped) and `vitest` (TagInput in the new editors, chip→filter). Update the `mcp/tools.rs` drift guard (match + `known` + count + `def`) in lockstep for `knowledge_tags`.

### B — Mindmaps (visual idea organization)

- [x] **14.5** Migration `0014_mindmaps.surql`: `mindmap` table mirroring `0005_documents.surql` — `uuid`/`owner`/`title`/`folder_id`/`project_id`/`tags`/`review`/`version`/`published_at?`/timestamps — plus a `graph` string body (validated JSON `{ nodes:[{ id, label, parent?, x?, y?, color?, item_type?, item_id? }], edges:[{ from, to, label? }] }`) and `source_format` (`json`|`markdown`). Indexes `mindmap_owner`/`mindmap_folder`/`mindmap_project` and a `mindmap` SEARCH section over `title` (node labels folded into a derived searchable text column, gated on the 11.1 capability check with the `CONTAINS` fallback).
- [x] **14.6** New `mindmap/` module (`mod/model/repo/routes`) mirroring `documents/`: owner-scoped CRUD with a `MINDMAP_SELECT` projection; **graph shape-validated** (unique node ids, edges reference existing nodes, label/node/edge caps, labels stored as plain text); node `item_type`/`item_id` validated to **owner-owned** items (mirror `ideas_link`'s existence checks); `kn_link` scrub on `delete_mindmap`; draft gate + `version` bump on a content edit only. A `from_markdown` builder (outline → tree) and a `from_project` seed (project ideas → nodes, `kn_link`s → edges, simple radial auto-layout). Fold a `mindmap` typed section into `knowledge::repo::search`.
- [x] **14.7** MCP `mindmaps_{list,get,create,update,delete}` + `mindmaps_from_project` (owner-scoped via the `owner` arg, mirroring `documents_*`); `mindmap.{create,update,delete}` arms in `activity::entry_for`. Update the `call()` match + `known` list + count assert + `def(…)` catalog **in the same commit**.
- [x] **14.8 (tests-with-features)** Frontend: lazy `MindmapsPage` using **`@xyflow/react`** (new dep) — interactive node/edge editing (drag, connect), import-from-Markdown, **seed-from-project**, node→item links that open the target feature page, and client-side **PNG/SVG export** plus the reusable `ExportMenu` (PDF/Markdown of an outline rendering via `POST /export`). Folder/breadcrumb + tags reused; `navItems` (phase 14) + `FEATURE_PAGES` wiring; `mindmaps/{api,types}.ts`. Backend tests (CRUD/validation/from_markdown/from_project/search/scrub, owner isolation) + `vitest` (editor renders a seeded graph, add/connect node, node link navigates, export calls the mutation).

### C — draw.io diagrams

- [x] **14.9** Migration `0015_diagrams.surql`: `diagram` table mirroring `document` (+ `tags`/`review`/`folder_id`/`project_id`/`version`/timestamps) with an `xml` body (mxGraph XML) and an optional `preview` (PNG/SVG persisted as an **owner-scoped file id** via the `files_write` pattern, or a capped `data:` URI). Indexes + a `diagram` SEARCH section over `title` (+ text labels extracted from the XML), gated on 11.1.
- [x] **14.10** New `diagrams/` module (`mod/model/repo/routes`) mirroring `documents/`: owner-scoped CRUD, an `xml` size cap (`MAX_BODY`), **`convert::sanitize` over the XML→SVG render** before any preview/export (scripts/`foreignObject` stripped), `kn_link` scrub on delete, draft gate; an export path (SVG/PNG/PDF via the existing `convert.rs` pathway). Add `DRAWIO_URL` config (default the hosted embed; **self-host documented in `.env.example`**) and a strict `frame-src <DRAWIO_URL>` CSP applied **only** to the diagram-editor route — never to `/p/*`.
- [x] **14.11** MCP `diagrams_{list,get,create,update,delete}` (create/update accept `xml`); `diagram.{create,update,delete}` arms in `activity::entry_for`; fold a `diagram` typed section into `knowledge_search`. Drift guard (match + `known` + count + `def`) updated in lockstep.
- [x] **14.12 (tests-with-features)** Frontend: lazy `DiagramsPage` embedding the draw.io editor (`<iframe>` `?embed=1&proto=json` + a `postMessage` load/save/export handler), persisting the XML + a rendered preview; a list with **static SVG/PNG** previews (never the live frame); the editor iframe is locked to `DRAWIO_URL` via CSP and is **absent from `/p/*`**. `diagrams/{api,types}.ts`; `navItems` + `FEATURE_PAGES` wiring (no new npm dep — embed mode is an iframe + postMessage). Backend tests (CRUD/sanitize-on-render/owner isolation/search) + `vitest` (editor mounts the iframe with the right embed URL, save persists XML, preview renders static SVG).

> **Milestones B + C shipped** (branch `phase-12-web-hosting`, working tree — verified green, **not yet committed**).
> 14.5/14.9: migrations `0014_mindmaps.surql` + `0015_diagrams.surql` — both document-like tables (folder/project/
> tags/review/version) with a typed body (`graph` JSON / mxGraph `xml`) + a derived `search_text` column and a
> `kn_text` SEARCH index pair. 14.6/14.10: new `src/mindmap/` + `src/diagrams/` modules (model/repo/routes) mirroring
> `documents/`/`pages/`; mindmap graphs are shape-validated (unique ids, edges/parents reference existing nodes, caps,
> node item-links checked against owner-owned items) with a Markdown-outline builder + a radial `seed_from_project`
> (root + member ideas + idea↔idea `kn_link` edges); diagram previews are guarded to `data:image/*` URIs; both scrub
> `kn_link`s on delete and bump `version` on content-only edits. `mindmap`/`diagram` are now first-class in the
> `knowledge` `ITEM_TYPES`/`MEMBER_TYPES` whitelists, `ProjectMembers`/`MemberCounts`, and `knowledge::repo::search`
> (two new typed sections). 14.7/14.11: eleven MCP tools (`mindmaps_{list,get,create,update,delete,from_project}` +
> `diagrams_{list,get,create,update,delete}`), five `mindmap.*`/`diagram.*` `activity::entry_for` arms, and the
> `call()` match + `known` list + count + `def(…)` catalog moved in lockstep — **58 MCP tools** total. 14.8/14.12:
> lazy `MindmapsPage` (`@xyflow/react` canvas, import-outline, seed-from-project, autosave) + `DiagramsPage` (draw.io
> `postMessage`/`?embed=1&proto=json` iframe persisting XML + a static SVG/PNG preview), wired in `App.tsx` +
> `navigation.ts` (phase 14). **As-built deltas:** (a) the draw.io origin is a **frontend** `VITE_DRAWIO_URL`
> (default `https://embed.diagrams.net`, documented in `.env.example`) rather than a backend `DRAWIO_URL` Config field —
> the backend never serves the SPA HTML, so a server-side `frame-src` CSP/Config knob would be unused and would force
> edits to 10 test `Config` literals; the `frame-src` lockdown is the SPA's index-CSP concern (a Phase-10/deploy item).
> (b) Diagram export reuses `POST /export` over the preview (no bespoke server-side draw.io→SVG render; the editor
> exports the SVG client-side). (c) The mindmap node→item-link *navigation* UI is deferred (links are stored + validated;
> the canvas doesn't yet open the target feature page). **Verified green: backend `cargo test` (152 — +16), `clippy
> -D warnings`, `fmt`; frontend `tsc`/`eslint`/`vitest` (61 — +6)/`vite build` (`MindmapsPage` code-split, ~59 KB gz).**
> Remaining: commit the working tree; a live browser screenshot of the canvas + draw.io flows. (Milestone A — cross-type
> tags — shipped earlier in the working tree per the note above.)

> Acceptance: **tags** round-trip on documents/pages (and ideas), normalized + deduped, filterable per feature, ANDable in `knowledge_search` (which now returns `tags`), aggregated by `GET /tags`/`knowledge_tags`, and shown as clickable chips. A **mindmap** is authored freehand / from a Markdown outline / **seeded from a project** (ideas→nodes, `kn_link`s→edges), its nodes link to Baitler items, it's filed in a folder + joined to a project + tagged + found by search + draft-gated + exportable. A **draw.io diagram** is authored in the embedded editor, stored as mxGraph XML with a **sanitized** SVG/PNG preview, filed/tagged/searchable/draft-gated, rendered as static SVG outside the editor and **never** as a live frame on `/p/*`. Owner-scoped; agent writes land as `review="draft"` + attributed in `activity`; the `mcp/tools.rs` drift guard moves with every new tool; backend `build`/`clippy -D warnings`/`test`/`fmt` + frontend `tsc`/`eslint`/`vitest`/`build` green; no test needs egress (draw.io is a client iframe, mindmap render is client-side, PDF export reuses the Chrome path already conditional in tests).
>
> **Deferred (build on real need):** real-time collaborative editing; mindmap/diagram **version history + diff**; a graph-DB representation (keep the JSON/XML body); a freeform whiteboard beyond mindmaps; bulk import of external `.drawio`/`.mmd` files; **tag hierarchies/synonyms/bulk-rename-with-propagation**, per-tag colors, and saved tag-filtered views (flat tags in v1); embedding *live* diagrams in published pages (static SVG only on `/p/*`); LLM **auto-tagging**/auto-mind-mapping (could reuse `ai_chat`); and extending `tags` to `file`/`project` (same column, when needed).
>
> **Dependencies & sequencing:** builds on **Phase 4** (folders), **Phase 7** (`convert.rs` export + the TipTap/editor patterns), **Phase 11** (projects/`kn_link`/`knowledge_search`/`review`/`activity`), and **Phase 12** (whose pages tag-deferral this reverses, and whose `knowledge_search` typed-section pattern this copies); the agent tools build on **Phase 13**'s loopback. All three milestones are independent and ship against the `DEV_OWNER` stub — nothing waits on Phase 2 beyond the usual multi-user caveats. **draw.io self-hosting is recommended for privacy**; the hosted embed is the zero-config default.

---

## Phase 15 — Superpage (composed knowledge canvas)

Goal: one **Superpage** is a first-class object where the user (and agents) arrange **blocks** that reference existing Baitler content — ideas, documents, pages, mindmaps, diagrams, files — plus local notes/headings, on a single full-pane canvas. It is the “everything I’m working on right now” surface: not a replacement for projects (organize) or pages (publish a narrative), but a **composition layer** with a single agent context bundle.

Vision: “Put what matters on one board.” A Superpage body is JSON `{ layout, blocks[] }` stored like a mindmap graph — document-shaped metadata (owner/title/folder/project/tags/review/version) with structure in the body, reusing folders, projects, `kn_link`, `knowledge_search`, review, activity, and MCP with no bespoke schema. Agents call `superpages_context` to fetch the layout plus **resolved** embed payloads in one shot instead of many `*_get` calls.

Architecture (additive): new `superpage` table (migration `0016`). Block kinds in MVP: **`embed`** (`item_type` + `item_id`, validated owner-owned), **`note`** (Markdown/plain text), **`heading`**. Caps: ≤50 blocks, layout coords optional (`x`/`y`/`w`/`h`). Embed types: `idea|document|file|page|mindmap|diagram` (not `project`/`superpage` in v1). Security: embed **previews** in the UI use the same rules as elsewhere — diagrams as static preview image, pages via sandboxed iframe or hardened snippet, ideas/docs as sanitized/plain text; never raw draw.io XML in the app origin.

- [x] **15.1** Migration `0016_superpages.surql`: `superpage` table mirroring `mindmap` (minus `source_format`; plus `blocks` JSON string + derived `search_text` from title + block notes/headings/embed titles).
- [x] **15.2** `superpage/` module (`mod/model/repo/routes/context`): owner-scoped CRUD; block validation; `kn_link` scrub on delete; `from_project` seeds a grid of embed blocks from project members; `GET /superpages/{id}/context` (and MCP `superpages_context`) resolves embeds with size caps.
- [x] **15.3** Fold `superpage` into `ITEM_TYPES`/`MEMBER_TYPES`, `project_members`/`member_counts`, and `knowledge_search` (typed section). MCP: `superpages_{list,get,create,update,delete,context,from_project}` + activity arms; drift guard updated in the same commit.
- [x] **15.4** Frontend: Objects → **Superpages** (`/superpages`, `/superpages/:id`); lazy `SuperpagesPage` with a grid canvas (add embed/note/heading, open-in-tab, autosave blocks). Immersive layout like mindmaps/diagrams on detail routes.
- [x] **15.5 (tests-with-features)** Backend integration tests (CRUD, validation, from_project, context resolver); frontend vitest smoke still TODO.

> **Branch:** `feature/superpage`. **Deferred:** public publish route for superpages; live embed refresh vs snapshot pins; `superpage`↔`superpage` embed; AI panel blocks; extending `review_list` to superpage drafts.

---

## Cross-cutting (apply continuously)

- **Typed API contract**: generate OpenAPI from the Rust API; derive frontend types so client and server never drift.
- **Tests-with-features**: each phase ships its own tests; don't defer to Phase 10.
- **Markdown everywhere**: ideas, documents, and AI output all support Markdown as the lingua franca.
- **Keep `CLAUDE.md` current**: update **Commands** and architecture notes as real code lands.
- **Provenance for agent-written content**: every MCP *mutation* records who (agent label) + what in
  the `activity` log; agent writes land as `review="draft"` pending human approval, never silently published.
- **MCP catalog drift guard**: a new tool updates the `call()` match, the `known` name list, and the
  `assert_eq!(advertised.len(), known.len())` count in `mcp/tools.rs` in the same commit (else `cargo test` reds).
- **Never serve user/agent HTML on the cookie origin**: any public-serving surface (Phase 12 `/p/*`)
  ships outside the credentialed CORS layer and any auth middleware, behind a no-script
  `default-src 'none'; sandbox` CSP, and on a distinct `PUBLIC_PAGE_ORIGIN` once Phase 2 adds session
  cookies — the auth cookie is never set on an origin that also serves public HTML.
- **Sanitize stored prose on write**: no draft/preview/search-snippet path ever renders unsanitized
  author/agent HTML in the app origin; server-side render paths additionally run `harden_for_render`.
- **Visual artifacts store structure in a body, not a new schema**: mindmaps (JSON graph) and diagrams
  (draw.io XML) are content types shaped like a `document` (owner/folder/project/tags/review/version + a
  typed body), so they reuse folders/projects/`kn_link`/`knowledge_search`/review/activity/export — never a
  bespoke graph schema. Each new searchable type folds a typed section into `knowledge_search`.
- **Third-party editor frames load only on the authoring surface**: the draw.io frame (Phase 14) is allowed via a
  scoped `frame-src <DRAWIO_URL>` on the editor route only — never on the public `/p/*` origin, which renders
  diagrams as static sanitized SVG. Self-hosting `DRAWIO_URL` is the privacy-safe deployment.
- **Never run the in-app agent unsandboxed**: the Claude Code CLI runner (Phase 13) is off by default,
  spawned headless with a server-built argv (no shell), host tools disallowed and `--dangerously-skip-permissions`
  never set, the Anthropic key in the child env (never argv/logs), in a per-run ephemeral sandbox dir; its
  loopback MCP writes inherit the `review="draft"` gate + `activity` attribution like any other agent.

## Suggested sequencing

Phases 0→3 are the critical path (you can't demo anything without repo + backend + auth + shell).
After Phase 3, Phases 4–8 are largely independent and can be parallelized or reordered by priority.
Phase 9 (mobile) waits until the API contract is stable (post Phase 4–6). Phase 10 runs last but
borrow its security/test items earlier where cheap. **Phase 7.5 (MCP foundation) shipped** with Phase 7;
**Phase 11 (Agentic Baitler)** builds directly on it — Milestone A (knowledge model + search + activity +
MVP agentic loop) can start now against the dev-owner stub; multi-agent identity and any public publishing
surface are gated on **Phase 2 auth**, so sequence those after it.
 **Phase 12 (Web page hosting)** realizes the `/p/:slug` surface Phase 11 deferred and reuses Phase 4/7/11
wholesale, so it can begin once Phase 11 merges; Milestones A–C ship against the `DEV_OWNER` stub, only
multi-user/per-link sharing (12.D) waits on Phase 2 — which must never set an auth cookie on an origin that serves `/p/*`.
 **Phase 13 (Claude Code CLI integration)** wraps the `claude` CLI as a sandboxed in-app agent runner that loops back
through `/mcp`, so it builds on Phase 6 (keys/SSE) + Phase 7.5/11 (the loopback target, activity, draft gate) and can
begin once Phase 11 merges; Milestones A–C ship against the `DEV_OWNER` stub with the Mock runner as the tested path
(real runs need egress + a `claude` binary + a key, absent here), and only multi-user quotas/auth (13.D) wait on Phase 2.
 **Phase 14 (Knowledge organization & visual modeling)** adds cross-type tags + mindmaps + draw.io diagrams; it reuses
Phase 4/7/11/12 wholesale (and Phase 13 for agent tools), so it can begin once Phase 11/12 merge. Its three milestones
(A tags, B mindmaps, C draw.io) are independent and ship against the `DEV_OWNER` stub; draw.io should be self-hosted
(`DRAWIO_URL`) for privacy, with the hosted embed as the zero-config default.
 **Phase 15 (Superpage)** builds on Phase 4/11/14 (folders, projects, all embeddable object types, MCP conventions);
ship on branch `feature/superpage` against the `DEV_OWNER` stub. Multi-user and public superpage URLs wait on Phase 2.
