# Baitler

> **Butler + AI** — a personal-assistant and data-organizer platform for managing your
> files, data, and ideas, with multi-provider LLM capabilities layered on top.
> Site: [baitler.com](https://baitler.com)

Baitler's **base portal** is the central hub where every feature comes together: file
storage, idea organization, data visualization, an HTML document editor with PDF/MS Office
export, and AI analysis across text, image, video, and audio.

## Stack

| Surface   | Tech |
|-----------|------|
| Frontend  | React + Vite + TypeScript + TailwindCSS |
| Backend   | Rust HTTP API (Axum) + SurrealDB |
| Mobile    | Native iOS (Swift) + Android (Kotlin) |
| Tooling   | `scripts/` for combined dev-run and builds |

## Repository layout

```
.
├── frontend/        # React + Vite + TS app (Phase 3)
├── backend/         # Rust API + SurrealDB (Phase 1)
├── mobile/
│   ├── ios/         # native iOS app (Phase 9)
│   └── android/     # native Android app (Phase 9)
├── scripts/         # dev-run + build orchestration
├── docs/            # user & developer docs
├── plan.md          # phased build plan (Phase 0–10)
└── Vibecoding/      # product brief (source of truth)
```

## Prerequisites

- **Node** — version pinned in [`.nvmrc`](.nvmrc) (`nvm use`)
- **Rust** — toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml)
- **SurrealDB** — install from <https://surrealdb.com/install>

## Quickstart

```bash
# 1. Configure environment
cp .env.example .env        # then fill in secrets/OAuth keys

# 2. Run the full dev stack (SurrealDB + backend + frontend)
./scripts/dev.sh

# 3. Build everything for release
./scripts/build.sh
```

> The dev/build scripts degrade gracefully: until the backend (Phase 1) and frontend
> (Phase 3) are scaffolded, they warn and skip rather than fail.

## Development status

This project is being built phase-by-phase per [`plan.md`](plan.md). **Phase 0**
(repository & tooling foundation) is in progress. See the plan for what each phase
delivers and the suggested sequencing.

## Configuration

All configuration is via environment variables — see [`.env.example`](.env.example)
for the full list (server, SurrealDB, OAuth providers, storage, LLM provider keys).
Never commit `.env`.
