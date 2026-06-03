-- 0012_cli_conversation — stable conversation id for multi-turn CLI chats (Phase 13).
--
-- A chat's turns must run in the SAME working directory, because the `claude` CLI
-- stores resumable sessions keyed to the cwd. `conversation_id` is a client-stable
-- id reused across a chat's turns; the runner maps it to a persistent `conv-<id>`
-- sandbox so `--resume <session_id>` finds the prior session.

DEFINE FIELD IF NOT EXISTS conversation_id ON cli_run TYPE option<string> DEFAULT NONE;
DEFINE INDEX IF NOT EXISTS cli_run_conversation ON cli_run FIELDS owner, conversation_id;
