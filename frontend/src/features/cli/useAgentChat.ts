import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { CLI_ROOT, streamRun, useCancelRun } from './api';
import type { CliRun, CreateRunRequest, RunEvent } from './types';

/** One exchange in the conversation: the user's prompt + the agent's events. */
export interface Turn {
  prompt: string;
  events: RunEvent[];
  streaming: boolean;
}

/** A stable id for a chat; all its turns share one working dir on the server. */
function newConversationId(): string {
  return (
    globalThis.crypto?.randomUUID?.() ?? `c-${Date.now()}-${Math.random().toString(36).slice(2)}`
  );
}

export function resultOf(events: RunEvent[]): Extract<RunEvent, { type: 'result' }> | undefined {
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i];
    if (e?.type === 'result') return e;
  }
  return undefined;
}

/** Per-send options: everything of a run request the caller controls, minus the
 * conversation plumbing (`prompt`/`resume_session_id`/`conversation_id`). */
export type SendOptions = Omit<
  CreateRunRequest,
  'prompt' | 'resume_session_id' | 'conversation_id'
>;

/**
 * The agent conversation state machine shared by the Butler home and the Agent
 * panel/dock: each `send` continues the same agent session (`--resume`) within a
 * stable conversation working dir, streaming events into the current turn.
 */
export function useAgentChat() {
  const qc = useQueryClient();
  const cancelRun = useCancelRun();

  const [turns, setTurns] = useState<Turn[]>([]);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [runId, setRunId] = useState<string | null>(null);
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [unavailable, setUnavailable] = useState<string | null>(null);

  const abortRef = useRef<AbortController | null>(null);

  // Abort a stream the moment the consumer unmounts (route change), so a
  // background reader doesn't keep updating unmounted state.
  useEffect(() => () => abortRef.current?.abort(), []);

  const send = async (text: string, opts: SendOptions) => {
    if (!text.trim() || streaming) return;

    // Establish (or reuse) the conversation so every turn shares one working dir.
    const convId = conversationId ?? newConversationId();
    if (!conversationId) setConversationId(convId);

    const index = turns.length;
    setTurns((prev) => [...prev, { prompt: text, events: [], streaming: true }]);
    setError(null);
    setUnavailable(null);
    setRunId(null);
    setStreaming(true);

    const controller = new AbortController();
    abortRef.current = controller;

    await streamRun(
      {
        ...opts,
        prompt: text,
        // Continue the conversation when we have a prior session; the shared
        // conversation_id keeps all turns in the same working dir so --resume works.
        resume_session_id: sessionId || undefined,
        conversation_id: convId,
      },
      {
        signal: controller.signal,
        onEvent: (ev) => {
          if (ev.type === 'run') {
            setRunId(ev.id);
            return;
          }
          if (ev.type === 'done') {
            void qc.invalidateQueries({ queryKey: CLI_ROOT });
            return;
          }
          if ((ev.type === 'init' || ev.type === 'result') && ev.session_id) {
            setSessionId(ev.session_id);
          }
          setTurns((prev) =>
            prev.map((t, i) => (i === index ? { ...t, events: [...t.events, ev] } : t)),
          );
        },
        onError: (message, statusCode) => {
          setError(message);
          if (statusCode === 503) setUnavailable(message);
        },
      },
    );

    setStreaming(false);
    abortRef.current = null;
    setTurns((prev) => prev.map((t, i) => (i === index ? { ...t, streaming: false } : t)));
    void qc.invalidateQueries({ queryKey: CLI_ROOT });
  };

  const stop = () => {
    if (runId) cancelRun.mutate(runId);
    else abortRef.current?.abort();
  };

  const reset = () => {
    if (streaming) return;
    setTurns([]);
    setSessionId(null);
    setConversationId(null);
    setRunId(null);
    setError(null);
    setUnavailable(null);
  };

  /** Seed the thread from a re-opened past run, so the next message continues it. */
  const seedFromRun = (run: CliRun) => {
    const events: RunEvent[] = [];
    if (run.result_text) events.push({ type: 'assistant', text: run.result_text });
    if (run.error) events.push({ type: 'error', message: run.error });
    events.push({
      type: 'result',
      text: run.result_text ?? '',
      session_id: run.session_id,
      num_turns: run.num_turns,
      cost_usd: run.cost_usd,
      is_error: run.status === 'failed',
    });
    setTurns([{ prompt: run.prompt, events, streaming: false }]);
    setSessionId(run.session_id);
    // Reuse the run's conversation (same working dir) so we truly resume it;
    // older runs without one fall back to a fresh conversation.
    setConversationId(run.conversation_id ?? newConversationId());
  };

  return {
    turns,
    sessionId,
    runId,
    streaming,
    error,
    unavailable,
    send,
    stop,
    reset,
    seedFromRun,
  };
}
