#!/usr/bin/env bash
# Build Baitler frontend and backend for release.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

built_any=false

# ── Backend (Rust API) ──
if [[ -f backend/Cargo.toml ]]; then
  echo "[build] backend: cargo build --release"
  ( cd backend && cargo build --release )
  built_any=true
else
  echo "[build] WARN: backend/Cargo.toml not found — skipping (Phase 1)."
fi

# ── Frontend (Vite) ──
if [[ -f frontend/package.json ]]; then
  echo "[build] frontend: npm ci && npm run build"
  ( cd frontend && npm ci && npm run build )
  built_any=true
else
  echo "[build] WARN: frontend/package.json not found — skipping (Phase 3)."
fi

if [[ "$built_any" == false ]]; then
  echo "[build] nothing to build yet."
  exit 0
fi

echo "[build] done."
