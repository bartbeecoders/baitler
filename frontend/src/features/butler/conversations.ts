import type { CliRunSummary } from '@/features/cli/types';

/** One past conversation: its runs folded into a selectable list entry. */
export interface Conversation {
  /** The shared conversation id (a run id for legacy single-run entries). */
  id: string;
  /** The opening prompt — the conversation's display title. */
  title: string;
  /** Newest run id — re-opening it resumes the latest session. */
  lastRunId: string;
  /** Newest activity timestamp. */
  updatedAt: string;
  /** Number of exchanges (runs) in the conversation. */
  runCount: number;
  /** Whether the newest run is still executing (re-open = live re-attach). */
  running: boolean;
}

/**
 * Fold the (newest-first) run history into conversations: runs sharing a
 * `conversation_id` are one chat; older runs without one stand alone. Titled by
 * the opening prompt, ordered by most recent activity.
 */
export function groupConversations(runs: CliRunSummary[]): Conversation[] {
  const byId = new Map<string, Conversation>();
  for (const run of runs) {
    const id = run.conversation_id ?? run.id;
    const existing = byId.get(id);
    if (existing) {
      // Older run of the same chat: it opened the conversation, so it titles it.
      existing.title = run.prompt;
      existing.runCount += 1;
    } else {
      byId.set(id, {
        id,
        title: run.prompt,
        lastRunId: run.id,
        updatedAt: run.updated_at,
        runCount: 1,
        running: run.status === 'running',
      });
    }
  }
  return [...byId.values()];
}
