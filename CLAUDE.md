# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project status: greenfield

No application code exists yet. As of this writing the entire repository is a single
product brief at `Vibecoding/instructions.md`. There is no build system, no installed
dependencies, and no git history. The bulk of the work here will be scaffolding the
project from scratch.

`Vibecoding/instructions.md` is the source of truth for scope and stack — read it
first. The sections below paraphrase it; if they ever disagree, the brief wins, and
this file should be updated once real code and commands exist.

## What Baitler is

Baitler (portmanteau of "Butler" + "AI"; baitler.com) is a personal-assistant and
data-organizer application for managing a user's files, data, and ideas, with AI/LLM
capabilities layered on top. The "base portal" is the central hub where every feature
comes together.

## Intended architecture (planned — not yet built)

A multi-surface system, implying a monorepo with separate workspaces sharing a
top-level `scripts/` directory:

- **Frontend** — React + Vite + TypeScript, styled with TailwindCSS.
- **Backend** — a Rust HTTP API, with **SurrealDB** as the datastore.
- **Mobile** — native iOS and Android apps.
- **`scripts/`** — dev orchestration: a script to run frontend + backend together,
  and build scripts for both. When these are created, document the concrete commands
  in a "Commands" section below.

## Features to support

- Base portal that unifies all functions
- User management with OAuth2 (Google, GitHub, etc.)
- File storage and management
- Idea management and organization
- Data visualization and analytics
- AI/LLM integration for analysis and insights:
  - external providers (OpenAI, Anthropic, OpenRouter, fal.ai, etc.)
  - multi-model support
  - multi-modal: text, image, video, audio
- HTML document editor and management
- Export: PDF and MS Office (Word, Excel, PowerPoint)
- Markdown support throughout

## Design notes grounded in the brief

- **LLM access is multi-provider, multi-model, multi-modal.** Build a provider
  abstraction up front rather than coding against one vendor's SDK — the brief names
  several providers and explicitly requires swapping models and modalities.
- **Document conversion is a recurring requirement.** HTML editing plus PDF and
  MS Office export plus Markdown suggests a single shared conversion/export pathway
  rather than per-feature one-offs.
- **Auth is OAuth2-first** (third-party identity providers), not username/password.

## Commands

The monorepo skeleton + scripts exist (Phase 0), the **backend is scaffolded**
(Phase 1: Cargo workspace at `backend/`, crate `baitler-api`), and the **frontend is
scaffolded** (Phase 3: Vite + React + TS at `frontend/`). Mobile (Phase 9) is not yet started.

- `cp .env.example .env` — create local config; fill in secrets/OAuth/LLM keys.
- `./scripts/dev.sh` — run the full dev stack together; Ctrl-C tears all down.
- `./scripts/build.sh` — release build (`cargo build --release` + `vite build`).

Backend (from `backend/`, or pass `--manifest-path backend/Cargo.toml`):

- `cargo run` — start the API. Defaults to `SURREAL_URL=rocksdb://./data/surreal.db`
  (embedded, **file-based** SurrealDB via the `kv-rocksdb` feature; data persists across
  restarts and the directory is auto-created), so **no `surreal` server or Docker is
  needed** for dev. `SURREAL_URL=memory` selects the ephemeral in-process engine instead.
  Endpoints: `GET /health` (DB-ping readiness), `GET /version`.
- `cargo test` — unit + integration tests (each passes `memory` explicitly for an
  ephemeral, isolated DB; ephemeral ports).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`.
- Bind host is `BIND_HOST` (not `HOST` — that collides with conda/build tooling).

**MCP server** (`src/mcp/`): Baitler is also an MCP server. The `baitler-api` process
serves MCP over **Streamable HTTP** at `POST /mcp` (in-process, reusing the same
repos/DB — JSON-RPC 2.0, JSON-response variant, `GET`/`DELETE`→405). A second binary
`baitler-mcp` (`src/bin/mcp_stdio.rs`) is a **stdio↔HTTP bridge** for clients that only
launch stdio servers; it forwards to a running server's `/mcp` and never opens the DB
(no RocksDB lock conflict). **70 tools** cover ideas/documents/files/folders/pages/mindmaps/diagrams/superpages/ai/export/health/review/cli-agent
plus the **Phase 11 knowledge layer** (`src/knowledge/`): `project` groupings (membership via
each item's `project_id`), a symmetric `kn_link` cross-type graph, BM25 full-text
`knowledge_search` (analyzer + per-field SEARCH indexes, migrations 0007/0008), and an
append-only `activity` provenance log (`src/activity.rs`, migration 0009). Agent writes default
to `review="draft"` for human approval. The **Phase 12 page-hosting layer** (`src/pages/`,
migration 0010) adds a `page` table (document-like sanitized HTML body + `slug`/`visibility`
draft|unlisted|public/`source_format`/`folder_id`/`project_id`) with `pages_*` tools and its own
`page` full-text section folded into `knowledge_search`; pages publish to a served `GET /p/{slug}`
(`src/pages/public.rs`) wired **outside** the credentialed CORS layer, body re-run through
`convert::harden_for_render` at serve time under a no-script `default-src 'none'; sandbox` CSP.
`pages_create` defaults to `visibility="draft"` (never self-published); `pages_publish` returns the
shareable URL and logs a `page.publish` activity row. Slug helpers live in `src/slug.rs`
(shared by projects + pages). The **Phase 14 visual-modeling layer** adds two more document-like
content types that reuse folders/projects/`kn_link`/`knowledge_search`/`review`/`activity` with no
bespoke graph schema: **mindmaps** (`src/mindmap/`, migration 0014) store a JSON node/edge `graph`
body (authored freehand, from a Markdown `outline`, or `mindmaps_from_project`-seeded from a
project's ideas + links) with `mindmaps_*` tools; **diagrams** (`src/diagrams/`, migration 0015)
store draw.io mxGraph `xml` + an optional sanitized `data:`-URI `preview` with `diagrams_*` tools.
Both fold a typed section into `knowledge_search` over a derived `search_text` (node/edge labels for
mindmaps, extracted `value=` labels for diagrams — never raw markup), are first-class in
`ITEM_TYPES`/`MEMBER_TYPES`, scrub `kn_link`s on delete, and default agent writes to `review="draft"`.
The frontend ships lazy `MindmapsPage` (`@xyflow/react` canvas) and `DiagramsPage` (the draw.io editor
in a `postMessage`/`?embed=1&proto=json` iframe loaded only on that authoring route, from
`VITE_DRAWIO_URL` — default `https://embed.diagrams.net`, self-hostable for privacy; previews render as
static `<img>` everywhere else). A `mcp::Actor{owner,agent}` (agent from `X-Baitler-Agent`)
is threaded from `handle` → `tools::call`; mutating tools log one activity row centrally. All
tools share the REST handlers' validation, owner-scoped to the dev owner until auth lands.
The MCP catalog drift guard (`call()` match + `known` list + count assert in `mcp/tools.rs`) must
move in lockstep when adding a tool. Config: `MCP_ENABLED` (default true), `MCP_AUTH_TOKEN`
(optional bearer, constant-time checked). Binary blobs (pdf/docx/file reads) are Base64 in the
JSON result. Full client setup (Claude Code, Hermes agent, other MCP tools) is in **`docs/mcp.md`**.

Frontend (from `frontend/`): `npm run dev` (Vite, port 8100), `npm run build`
(`tsc -b` + `vite build`), `npm run lint`, `npm run typecheck`, `npm test` (Vitest).
Stack: React 19 + TS (strict) + Tailwind v4 + React Router 7 + TanStack Query 5 + Zustand;
react-markdown + @tailwindcss/typography for Markdown. Heavy feature routes are lazy-loaded.
Features built: base portal, Files (Phase 4), Ideas (Phase 5, reusable MarkdownEditor),
AI (Phase 6: multi-provider chat via a Mock + OpenAI/OpenRouter/Anthropic adapter behind an
`LlmProvider` trait; SSE streaming; per-owner API keys encrypted at rest with `APP_SECRET`),
Documents (Phase 7: TipTap HTML editor + shared conversion pathway in `src/convert.rs` —
Markdown↔HTML, PDF via headless Chrome `CHROME_BIN`, Word via Pandoc `PANDOC_BIN` if present;
HTML sanitized with ammonia, and `convert::harden_for_render` strips remote resources before a
server-side render to close the SSRF surface; `POST /export` is the reusable export endpoint),
Projects (Phase 11: lazy `ProjectsPage` portal over `knowledge/routes.rs` — Projects/Review/Activity
tabs, draft-approval queue, provenance badges),
Pages (Phase 12: lazy `PagesPage` — authors pages with the reused TipTap editor, filters by
visibility/folder/`q`, a publish/visibility toggle with copy-share-link, and previews **only** via a
sandboxed `<iframe>` against the cross-origin `/p/{slug}` serve route, never by injecting page HTML),
Mindmaps (Phase 14: lazy `MindmapsPage` — a `@xyflow/react` node/edge canvas, import-from-outline,
seed-from-project, autosaving the graph), Diagrams (Phase 14: lazy `DiagramsPage` — the draw.io editor
embedded via `postMessage`, persisting mxGraph XML + a static SVG/PNG preview).
**Sidebar "Objects" layout:** the content types (Documents/Ideas/Pages/Mindmaps/Diagrams) are
not flat nav links — they live in an expandable **"Objects"** group in the left `Sidebar`
(`src/features/objects/`: a generic `ObjectList` with +/search/filter/refresh/trash controls, per-type
`adapters.tsx`, the `ObjectsNav` accordion, expansion state in `stores/objectsNav.ts`). Selecting an
item deep-links to a detail route, so each of those feature **pages renders editor-only** (`/{base}` →
an `EmptyDetail` hint, `/{base}/:id` → the editor); Ideas opens its modal at `/ideas/:id` and
`/ideas/new`. Files/Projects/AI/Agent stay flat nav. (Known minor follow-up: tapping an Objects item on
mobile doesn't auto-close the drawer.)
No LLM egress/keys in this env — the Mock provider is the tested path; real adapters are
compiled but unexercised. PDF export needs Chrome; Word needs Pandoc (absent here → 503).
Set `APP_SECRET` in production.
`@` aliases `src/`; Vite reads `VITE_*` from the **repo-root** `.env` (`envDir`).
Auth/OAuth and route guards are deferred to the final phase — the shell is auth-ready
(credentialed API client + `UserMenu` stub) but has no login yet.

CI (`.github/workflows/ci.yml`) runs fmt/clippy/test for the backend and
lint/typecheck/build/test for the frontend, skipping a service until it is scaffolded.

Toolchain versions are pinned: Node in `.nvmrc`, Rust in `rust-toolchain.toml`.
