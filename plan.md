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

- [ ] **6.1** Define a provider-neutral `LlmProvider` trait: `complete`, `chat`, `embed`, plus modality capabilities (text/image/video/audio).
- [ ] **6.2** Implement adapters: OpenAI, Anthropic, OpenRouter, fal.ai. Per-user/per-org API key storage (encrypted at rest).
- [ ] **6.3** Model registry + selection (multi-model): list available models per provider, route requests, capture usage/cost.
- [ ] **6.4** Streaming responses (SSE/websocket) to the frontend.
- [ ] **6.5** Multi-modal request/response handling (image, audio, video inputs/outputs) where providers support it.
- [ ] **6.6** Feature surfaces: chat-with-your-data, summarize a file/idea, generate insights; ground prompts in user content (RAG via SurrealDB embeddings/vector search).
- [ ] **6.7** Frontend: AI chat panel, model/provider picker, streaming UI, attach files/data as context.
- [ ] **6.8** Tests with mocked providers; guardrails for key handling, rate limits, and timeouts.

## Phase 7 — HTML document editor & conversion/export pathway

Goal: rich document editing plus the shared HTML ↔ Markdown ↔ PDF ↔ Office pipeline.

- [ ] **7.1** Choose rich-text/HTML editor (TipTap/ProseMirror); `Document` schema (HTML/Markdown body, version, owner).
- [ ] **7.2** Editor UI: formatting toolbar, Markdown import/export, autosave, document list/management.
- [ ] **7.3** **Shared conversion service** (single pathway, server-side): HTML↔Markdown, and export to:
  - [ ] PDF (headless Chromium / `wkhtmltopdf` / typst).
  - [ ] MS Word (`.docx`), Excel (`.xlsx`), PowerPoint (`.pptx`) — via Pandoc/LibreOffice headless or Rust crates.
- [ ] **7.4** Export endpoints returning the chosen format; progress/async for large jobs.
- [ ] **7.5** Frontend export menu (PDF / Word / Excel / PowerPoint / Markdown) reused across editor, ideas, and files.
- [ ] **7.6** Tests: round-trip conversions and export fidelity smoke tests.

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

---

## Cross-cutting (apply continuously)

- **Typed API contract**: generate OpenAPI from the Rust API; derive frontend types so client and server never drift.
- **Tests-with-features**: each phase ships its own tests; don't defer to Phase 10.
- **Markdown everywhere**: ideas, documents, and AI output all support Markdown as the lingua franca.
- **Keep `CLAUDE.md` current**: update **Commands** and architecture notes as real code lands.

## Suggested sequencing

Phases 0→3 are the critical path (you can't demo anything without repo + backend + auth + shell).
After Phase 3, Phases 4–8 are largely independent and can be parallelized or reordered by priority.
Phase 9 (mobile) waits until the API contract is stable (post Phase 4–6). Phase 10 runs last but
borrow its security/test items earlier where cheap.
