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

- [ ] **1.1** `cargo init backend` as a workspace; pick web framework (recommend **Axum** + Tokio).
- [ ] **1.2** Add core deps: `axum`, `tokio`, `tower-http` (CORS/trace), `serde`, `surrealdb`, `tracing`, `thiserror`, `config`/`dotenvy`.
- [ ] **1.3** Config loading from env (`PORT`, `SURREAL_URL`, `SURREAL_NS`, `SURREAL_DB`, secrets).
- [ ] **1.4** SurrealDB connection pool + a `migrations/` or schema-init module; run a local SurrealDB via Docker/`scripts/dev.sh`.
- [ ] **1.5** `GET /health` and `GET /version` endpoints; structured logging + request tracing middleware.
- [ ] **1.6** Standard error type → JSON error responses; CORS configured for the frontend origin.
- [ ] **1.7** Integration test harness (spin up app + test DB namespace) with one passing test.

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

- [ ] **3.1** Scaffold `frontend` with Vite + React + TS; install + configure TailwindCSS.
- [ ] **3.2** App shell: routing (React Router), layout (nav/sidebar/header), theme (light/dark), design tokens.
- [ ] **3.3** API client layer (typed fetch/axios wrapper, auth header/cookie handling, error toasts); pick data-fetching lib (TanStack Query).
- [ ] **3.4** **Base portal** dashboard: landing hub with cards/sections linking to each feature (files, ideas, analytics, editor, AI).
- [ ] **3.5** Shared UI kit (buttons, inputs, modals, tables) + state management (Zustand/Context).
- [ ] **3.6** Wire the Phase 2 auth flow into the shell; protected layout.

## Phase 4 — File storage & management

Goal: upload, browse, organize, download files.

- [ ] **4.1** Choose storage backend (local disk for dev, S3-compatible/object store for prod) behind a storage trait.
- [ ] **4.2** `File`/`Folder` schema (owner, path/hierarchy, mime, size, metadata, timestamps).
- [ ] **4.3** Upload endpoint (streamed/multipart, size limits), download endpoint (signed/streamed), delete, rename, move.
- [ ] **4.4** Folder/tree organization + listing with pagination and search.
- [ ] **4.5** Frontend file manager: tree/grid view, drag-and-drop upload, preview (images/PDF/text), context actions.
- [ ] **4.6** Tests for upload/download/permissions (a user only sees their own files).

## Phase 5 — Idea management & organization

Goal: capture, tag, link, and organize ideas/notes.

- [ ] **5.1** `Idea`/`Note` schema (title, body in Markdown, tags, links, status, timestamps); leverage SurrealDB graph relations for linking.
- [ ] **5.2** CRUD endpoints + full-text/tag search and filtering.
- [ ] **5.3** Frontend: idea list/board (list + kanban/graph views), tag management, linking between ideas.
- [ ] **5.4** Markdown editing/preview in the idea editor (shared with Phase 7 editor stack).
- [ ] **5.5** Tests for CRUD, tagging, and relations.

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
