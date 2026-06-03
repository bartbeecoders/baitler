-- 0011_cli_runs — Claude Code CLI runs (Phase 13).
--
-- One row per invocation of the in-app agent runner (`POST /cli/runs`). The row
-- records the user's prompt and the run's terminal outcome (status, resumable
-- session id, turns, cost) — never verbatim tool bodies. `status`:
--   running   — streaming now (process alive)
--   succeeded — agent returned a result, not an error
--   failed    — spawn/parse/timeout error, or the agent reported an error
--   cancelled — explicitly cancelled via POST /cli/runs/{id}/cancel
-- `tool_scope` is kb_only | kb_plus_read. `project_id` scopes the run's loopback
-- MCP writes to a Phase 11 project. Owner-scoped like everything else.

DEFINE TABLE IF NOT EXISTS cli_run SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS uuid ON cli_run TYPE string;
DEFINE FIELD IF NOT EXISTS owner ON cli_run TYPE string;
DEFINE FIELD IF NOT EXISTS prompt ON cli_run TYPE string;
DEFINE FIELD IF NOT EXISTS model ON cli_run TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS tool_scope ON cli_run TYPE string DEFAULT "kb_only";
DEFINE FIELD IF NOT EXISTS status ON cli_run TYPE string DEFAULT "running"; -- running|succeeded|failed|cancelled
DEFINE FIELD IF NOT EXISTS session_id ON cli_run TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS num_turns ON cli_run TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS cost_usd ON cli_run TYPE option<float> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS exit_code ON cli_run TYPE option<int> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS result_text ON cli_run TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS error ON cli_run TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS project_id ON cli_run TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS created_at ON cli_run TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON cli_run TYPE datetime VALUE time::now();
DEFINE FIELD IF NOT EXISTS finished_at ON cli_run TYPE option<datetime> DEFAULT NONE;

DEFINE INDEX IF NOT EXISTS cli_run_uuid ON cli_run FIELDS uuid UNIQUE;
DEFINE INDEX IF NOT EXISTS cli_run_owner ON cli_run FIELDS owner, created_at;
