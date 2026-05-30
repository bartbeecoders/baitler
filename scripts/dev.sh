#!/usr/bin/env bash
# Run the full Baitler dev stack: SurrealDB + backend API + frontend.
# Each service runs in the background; Ctrl-C tears all of them down.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Load .env if present (without overriding already-exported vars).
if [[ -f .env ]]; then
  set -a; source .env; set +a
fi

pids=()
cleanup() {
  echo ""
  echo "[dev] shutting down…"
  for pid in "${pids[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup INT TERM EXIT

# ── SurrealDB ──
# The backend embeds SurrealDB and, by default, opens a file-based RocksDB store
# itself (SURREAL_URL=rocksdb://./data/surreal.db) — no separate server needed.
# Only spin up a standalone `surreal` server when SURREAL_URL points at one
# (ws:// or http://); otherwise a server on the same rocksdb path would collide
# with the backend on RocksDB's exclusive lock.
if [[ "${SURREAL_URL:-}" == ws://* || "${SURREAL_URL:-}" == wss://* \
   || "${SURREAL_URL:-}" == http://* || "${SURREAL_URL:-}" == https://* ]]; then
  if command -v surreal >/dev/null 2>&1; then
    echo "[dev] starting SurrealDB server (SURREAL_URL=${SURREAL_URL})"
    surreal start --user "${SURREAL_USER:-root}" --pass "${SURREAL_PASS:-root}" \
      "rocksdb:./data/surreal.db" &
    pids+=($!)
  else
    echo "[dev] WARN: SURREAL_URL is a server URL but 'surreal' is not on PATH — skipping DB."
    echo "        Install: https://surrealdb.com/install"
  fi
else
  echo "[dev] using embedded SurrealDB (SURREAL_URL=${SURREAL_URL:-rocksdb://./data/surreal.db})"
fi

# ── Backend (Rust API) ──
if [[ -f backend/Cargo.toml ]]; then
  echo "[dev] starting backend (cargo run)"
  ( cd backend && cargo run ) &
  pids+=($!)
else
  echo "[dev] WARN: backend/Cargo.toml not found — backend not scaffolded yet (Phase 1)."
fi

# ── Frontend (Vite) ──
if [[ -f frontend/package.json ]]; then
  echo "[dev] starting frontend (npm run dev)"
  ( cd frontend && npm run dev ) &
  pids+=($!)
else
  echo "[dev] WARN: frontend/package.json not found — frontend not scaffolded yet (Phase 3)."
fi

if [[ ${#pids[@]} -eq 0 ]]; then
  echo "[dev] nothing to run yet. Scaffold backend (Phase 1) / frontend (Phase 3) first."
  exit 0
fi

echo "[dev] all services started. Press Ctrl-C to stop."
wait
