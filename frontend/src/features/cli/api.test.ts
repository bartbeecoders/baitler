import { afterEach, describe, expect, it, vi } from 'vitest';

import { streamRun } from './api';
import type { RunEvent } from './types';

function sseResponse(body: string, status = 200): Response {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(body));
      controller.close();
    },
  });
  return new Response(stream, { status });
}

describe('streamRun', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('parses the run-event stream into typed events', async () => {
    const sse = [
      'data: {"type":"run","id":"r1"}',
      'data: {"type":"init","session_id":"s1","model":"m1"}',
      'data: {"type":"assistant","text":"hi"}',
      'data: {"type":"tool_use","name":"mcp__baitler__ideas_create","summary":"create"}',
      'data: {"type":"result","text":"done","session_id":"s1","num_turns":2,"cost_usd":0,"is_error":false}',
      'data: {"type":"done","status":"succeeded"}',
      '',
    ].join('\n\n');
    vi.stubGlobal('fetch', vi.fn(async () => sseResponse(sse)));

    const events: RunEvent[] = [];
    let error: string | null = null;
    await streamRun(
      { prompt: 'do it' },
      { onEvent: (e) => events.push(e), onError: (m) => (error = m) },
    );

    expect(error).toBeNull();
    expect(events.map((e) => e.type)).toEqual([
      'run',
      'init',
      'assistant',
      'tool_use',
      'result',
      'done',
    ]);
  });

  it('surfaces a 503 (disabled runner) via onError with the status', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () =>
        sseResponse('{"error":{"code":"unavailable","message":"runner disabled"}}', 503),
      ),
    );

    let message: string | null = null;
    let status: number | undefined;
    await streamRun(
      { prompt: 'x' },
      {
        onEvent: () => {},
        onError: (m, s) => {
          message = m;
          status = s;
        },
      },
    );
    expect(message).toBe('runner disabled');
    expect(status).toBe(503);
  });
});
