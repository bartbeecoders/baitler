/** Claude Code CLI runner API shapes (mirror the backend DTOs, Phase 13). */

export type ToolScope = 'kb_only' | 'kb_plus_read';

export type AgentProvider = 'claude_code' | 'minimax';

/** A selectable agent backend + whether it can run right now (GET /cli/status). */
export interface ProviderStatus {
  id: AgentProvider;
  label: string;
  available: boolean;
  detail: string;
}

export type RunStatus = 'running' | 'succeeded' | 'failed' | 'cancelled';

/** A streamed run event (the `type`-tagged SSE messages from POST /cli/runs). */
export type RunEvent =
  | { type: 'run'; id: string }
  | { type: 'init'; session_id: string | null; model: string | null }
  | { type: 'assistant'; text: string }
  | { type: 'tool_use'; name: string; summary: string }
  | { type: 'tool_result'; ok: boolean; summary: string }
  | {
      type: 'result';
      text: string;
      session_id: string | null;
      num_turns: number;
      cost_usd: number | null;
      is_error: boolean;
    }
  | { type: 'error'; message: string }
  | { type: 'done'; status: RunStatus };

/** Lightweight run entry for the history list. */
export interface CliRunSummary {
  id: string;
  prompt: string;
  model: string | null;
  tool_scope: ToolScope;
  status: RunStatus;
  session_id: string | null;
  conversation_id: string | null;
  project_id: string | null;
  created_at: string;
  updated_at: string;
}

/** Full run detail. */
export interface CliRun extends CliRunSummary {
  session_id: string | null;
  num_turns: number;
  cost_usd: number | null;
  exit_code: number | null;
  result_text: string | null;
  error: string | null;
  finished_at: string | null;
}

/** Run-free readiness for the Agent page (GET /cli/status). */
export interface CliStatus {
  enabled: boolean;
  kind: string;
  binary_ok: boolean;
  version: string | null;
  has_stored_key: boolean;
  host_key_env: boolean;
  ready: boolean;
  message: string;
  providers: ProviderStatus[];
  /** Allow-listed local roots a run may be granted read access to (empty = off). */
  workspace_roots: string[];
}

export interface CreateRunRequest {
  prompt: string;
  provider?: AgentProvider;
  /** Orienting context appended to the system prompt (e.g. the current page). */
  context?: string;
  /** A local folder to grant the run read-only access to (within an allowed root). */
  workspace_dir?: string;
  model?: string;
  project_id?: string;
  tool_scope?: ToolScope;
  max_turns?: number;
  resume_session_id?: string;
  /** Stable chat id: all turns share it so they run in the same working dir. */
  conversation_id?: string;
}
