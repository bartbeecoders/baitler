# Baitler Redesign v2 — Butler-first home

Branch `redesign-v2`. Where the first `redesign` branch only re-worded the existing
Agent page around the butler story, v2 restructures the app so **the butler IS the
home page** and the rest of the UI steps back into a thin icon rail.

## The model

> "I ask it to look at a code project / a folder with documents; it reads,
> understands, summarizes, reorganizes into Baitler's structure, archives,
> and annotates/tags/categorizes everything it processes."

That loop is now the first thing you see:

```
┌──┬─────────────────────────────────────────────┐
│☰ │   "Good evening — what shall I organize     │
│🏠│    for you?"                                │
│📁│  ┌───────────────────────────────────────┐  │
│⬡ │  │ 📎 myproject (read-only)              │  │
│🗂 │  │ > summarize this code project…   [Go] │  │
│✨│  └───────────────────────────────────────┘  │
│🤖│  [Summarize a code project] [Ingest docs]   │
│  │  ───────────── Butler report ─────────────  │
│  │  ✅ "analyze acme-app"           2h ago     │
│  │     💡4 ideas 📄1 document 📦12 files       │
│  │     ↳ acme-app summary · auth notes · …     │
│  │  ⚠ 3 drafts awaiting your review →          │
└──┴─────────────────────────────────────────────┘
```

## Backend (new)

- **Run↔activity correlation** — migration `0017_activity_run.surql` adds
  `activity.run_id`. The CLI runner's loopback MCP config now sends
  `X-Baitler-Run: <run uuid>` next to `X-Baitler-Agent`; `mcp::Actor` carries it
  and every mutating tool call's activity row records it. `GET /activity` and the
  `activity_list` tool accept a `run_id` filter.
- **`GET /cli/runs/{id}/report`** — the "butler report": the run summary + its
  activity rows (artifacts, newest first) + counts grouped by action
  (`idea.create: 4, file.import: 12, …`). Exact provenance, not a time-window
  heuristic.
- **`GET /cli/workspace/browse[?path=…]`** — server-side folder listing strictly
  under `CLAUDE_CLI_WORKSPACE_ROOTS` (same `resolve_under_roots` canonicalization
  as a workspace grant; hidden entries and symlinks skipped, 200-entry cap,
  parent offered only while still inside a root). Powers the folder picker.

## Frontend (new)

- **`features/butler/`** — `ButlerHome` (the `/` route, lazy):
  - greeting hero + a large command composer (Enter to send, multi-turn via the
    same conversation/`--resume` plumbing as the Agent page);
  - **Attach a folder**: `WorkspacePicker` browses the allow-listed roots via the
    new endpoint and grants the chosen dir read-only (auto-switches the scope to
    `kb_plus_read`); shown as a removable 📎 chip;
  - **Quick tasks** (`quickTasks.ts`): summarize-a-code-project,
    ingest-&-organize-docs (both open the picker), tidy-my-knowledge-base;
  - live transcript reusing the shared `EventRow`;
  - **`RunReportFeed`**: recent runs as cards — status, relative time, count
    badges (`reportBadges`), deep links to created artifacts
    (`/ideas/:id`, `/editor/:id`, …), a "drafts awaiting review" banner.
- **Icon rail** — `components/layout/Rail.tsx` replaces the 288-px sidebar on
  desktop (`w-16`); content types open in an **Objects flyout** panel (reusing
  `ObjectsNav` + the adapters). Mobile keeps the full drawer (`Sidebar`).
- **Shared agent chat** — the conversation state machine was extracted from
  `AgentPanel` into `features/cli/useAgentChat.ts` (+ `EventRow.tsx`), so the
  butler home, the `/agent` page, and the dock all share one implementation.
- The agent dock stays available on every other page but is suppressed on `/`
  (the butler home is itself a full agent surface) as well as `/agent`.
- The old `Dashboard` (feature-card grid) is gone; `SystemStatus` moved to the
  bottom of the butler home. The `/` nav item is now **Butler** (concierge-bell).

## Guardrails (unchanged)

- Host writes stay denied by default; granted folders are read-only and
  allow-listed server-side; agent KB writes default to `review=draft`; every
  mutation is activity-attributed (now per-run, too).

## Follow-ups

- Re-open a report card's conversation directly (seed the chat from the run).
- Per-run tag chips on report cards (tags aren't in activity rows yet).
- Remembered/recent folder grants; multi-folder attach.
- An "archive plan" artifact the butler emits after an ingest (safe `mv` list).
