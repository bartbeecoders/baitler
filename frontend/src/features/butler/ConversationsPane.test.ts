import { describe, expect, it } from 'vitest';

import type { CliRunSummary } from '@/features/cli/types';
import { groupConversations } from './conversations';

function run(over: Partial<CliRunSummary>): CliRunSummary {
  return {
    id: 'r1',
    prompt: 'Do something',
    model: null,
    tool_scope: 'kb_only',
    status: 'succeeded',
    session_id: 's1',
    conversation_id: 'c1',
    project_id: null,
    created_at: '2026-06-05T10:00:00Z',
    updated_at: '2026-06-05T10:00:00Z',
    ...over,
  };
}

describe('groupConversations', () => {
  it('folds newest-first runs sharing a conversation_id into one entry', () => {
    const groups = groupConversations([
      run({ id: 'r3', conversation_id: 'c1', prompt: 'And one more thing', updated_at: '2026-06-05T12:00:00Z' }),
      run({ id: 'r2', conversation_id: 'c1', prompt: 'Now refine it', updated_at: '2026-06-05T11:00:00Z' }),
      run({ id: 'r1', conversation_id: 'c1', prompt: 'Summarize my folder', updated_at: '2026-06-05T10:00:00Z' }),
    ]);

    expect(groups).toHaveLength(1);
    const [c] = groups;
    expect(c?.id).toBe('c1');
    // Titled by the OPENING prompt, resumed via the NEWEST run.
    expect(c?.title).toBe('Summarize my folder');
    expect(c?.lastRunId).toBe('r3');
    expect(c?.updatedAt).toBe('2026-06-05T12:00:00Z');
    expect(c?.runCount).toBe(3);
  });

  it('keeps separate conversations apart, ordered by latest activity', () => {
    const groups = groupConversations([
      run({ id: 'r4', conversation_id: 'c2', prompt: 'Newer chat' }),
      run({ id: 'r3', conversation_id: 'c1', prompt: 'Older chat, second turn' }),
      run({ id: 'r1', conversation_id: 'c1', prompt: 'Older chat' }),
    ]);

    expect(groups.map((c) => c.id)).toEqual(['c2', 'c1']);
    expect(groups.map((c) => c.title)).toEqual(['Newer chat', 'Older chat']);
  });

  it('marks a conversation running from its NEWEST run only', () => {
    const groups = groupConversations([
      run({ id: 'r2', conversation_id: 'c1', status: 'running' }),
      run({ id: 'r1', conversation_id: 'c1', status: 'succeeded' }),
      run({ id: 'r0', conversation_id: 'c0', status: 'succeeded' }),
    ]);

    expect(groups.map((c) => c.running)).toEqual([true, false]);
  });

  it('treats legacy runs without a conversation_id as standalone chats', () => {
    const groups = groupConversations([
      run({ id: 'r2', conversation_id: null, prompt: 'Legacy two' }),
      run({ id: 'r1', conversation_id: null, prompt: 'Legacy one' }),
    ]);

    expect(groups.map((c) => c.id)).toEqual(['r2', 'r1']);
    expect(groups.every((c) => c.runCount === 1)).toBe(true);
  });
});
