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

- `cargo run` — start the API. Defaults to `SURREAL_URL=memory` (embedded, in-process
  SurrealDB), so **no `surreal` server or Docker is needed** for dev/tests. Endpoints:
  `GET /health` (DB-ping readiness), `GET /version`.
- `cargo test` — 18 unit + integration tests (embedded `memory` DB, ephemeral ports).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`.
- Bind host is `BIND_HOST` (not `HOST` — that collides with conda/build tooling).

Frontend (from `frontend/`): `npm run dev` (Vite, port 5173), `npm run build`
(`tsc -b` + `vite build`), `npm run lint`, `npm run typecheck`, `npm test` (Vitest).
Stack: React 19 + TS (strict) + Tailwind v4 + React Router 7 + TanStack Query 5 + Zustand;
react-markdown + @tailwindcss/typography for Markdown. Heavy feature routes are lazy-loaded.
Features built: base portal, Files (Phase 4), Ideas (Phase 5, reusable MarkdownEditor),
AI (Phase 6: multi-provider chat via a Mock + OpenAI/OpenRouter/Anthropic adapter behind an
`LlmProvider` trait; SSE streaming; per-owner API keys encrypted at rest with `APP_SECRET`),
Documents (Phase 7: TipTap HTML editor + shared conversion pathway in `src/convert.rs` —
Markdown↔HTML, PDF via headless Chrome `CHROME_BIN`, Word via Pandoc `PANDOC_BIN` if present;
HTML sanitized with ammonia; `POST /export` is the reusable export endpoint).
No LLM egress/keys in this env — the Mock provider is the tested path; real adapters are
compiled but unexercised. PDF export needs Chrome; Word needs Pandoc (absent here → 503).
Set `APP_SECRET` in production.
`@` aliases `src/`; Vite reads `VITE_*` from the **repo-root** `.env` (`envDir`).
Auth/OAuth and route guards are deferred to the final phase — the shell is auth-ready
(credentialed API client + `UserMenu` stub) but has no login yet.

CI (`.github/workflows/ci.yml`) runs fmt/clippy/test for the backend and
lint/typecheck/build/test for the frontend, skipping a service until it is scaffolded.

Toolchain versions are pinned: Node in `.nvmrc`, Rust in `rust-toolchain.toml`.
