-- 0017_activity_run — correlate activity rows to the CLI run that caused them
-- (redesign v2: the butler report). `run_id` is the `cli_run.uuid` carried on the
-- loopback MCP call via the `X-Baitler-Run` header; NONE for human/web actions
-- and for MCP clients that aren't a Baitler-spawned run.

DEFINE FIELD IF NOT EXISTS run_id ON activity TYPE option<string> DEFAULT NONE;
DEFINE INDEX IF NOT EXISTS activity_run ON activity FIELDS owner, run_id;
