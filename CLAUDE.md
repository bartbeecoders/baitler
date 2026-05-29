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

The monorepo skeleton and orchestration scripts exist (Phase 0). The frontend
(Phase 3) and backend (Phase 1) are not scaffolded yet, so the scripts warn and skip
those services until their manifests appear.

- `cp .env.example .env` — create local config; fill in secrets/OAuth/LLM keys.
- `./scripts/dev.sh` — run the full dev stack (SurrealDB + backend `cargo run` +
  frontend `npm run dev`) together; Ctrl-C tears all down.
- `./scripts/build.sh` — release build (`cargo build --release` + `vite build`).

Per-service commands (run inside the workspace dir) once scaffolded:

- Backend: `cargo run`, `cargo test`, `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`.
- Frontend: `npm run dev`, `npm run build`, `npm run lint`, `npm test`.

CI (`.github/workflows/ci.yml`) runs fmt/clippy/test for the backend and
lint/typecheck/build/test for the frontend, skipping a service until it is scaffolded.

Toolchain versions are pinned: Node in `.nvmrc`, Rust in `rust-toolchain.toml`.
