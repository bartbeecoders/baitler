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
- [ ] **11.12** Frontend (thin): a Projects page (project cards with member + draft-pending counts) and a Project
  detail view that lists member documents/ideas/files grouped by type, each linking to its existing Files/Ideas/
  Documents feature page, with a provenance line ("created by claude-code") and a Draft/Published chip; a Review
  queue filtering `review=draft` ideas+documents with inline Approve (PATCH → published) and Reject/Delete (reuse the
  Phase 4 accessible ConfirmModal); an Activity timeline (`GET /activity`). Reuse the existing `MarkdownEditor` and
  `ExportMenu` (a project README exports via `POST /export`). Lazy-loaded and nav-registered like the other features.

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

## Suggested sequencing

Phases 0→3 are the critical path (you can't demo anything without repo + backend + auth + shell).
After Phase 3, Phases 4–8 are largely independent and can be parallelized or reordered by priority.
Phase 9 (mobile) waits until the API contract is stable (post Phase 4–6). Phase 10 runs last but
borrow its security/test items earlier where cheap. **Phase 7.5 (MCP foundation) shipped** with Phase 7;
**Phase 11 (Agentic Baitler)** builds directly on it — Milestone A (knowledge model + search + activity +
MVP agentic loop) can start now against the dev-owner stub; multi-agent identity and any public publishing
surface are gated on **Phase 2 auth**, so sequence those after it.
