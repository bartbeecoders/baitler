import { afterEach, describe, expect, it, vi } from 'vitest';

import { streamChat } from './api';

function sseResponse(body: string): Response {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(body));
      controller.close();
    },
  });
  return new Response(stream, { status: 200 });
}

describe('streamChat', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('parses SSE delta events into text chunks', async () => {
    const sse = 'data: {"delta":"Hel"}\n\ndata: {"delta":"lo"}\n\ndata: {"done":true}\n\n';
    vi.stubGlobal('fetch', vi.fn(async () => sseResponse(sse)));

    const deltas: string[] = [];
    let error: string | null = null;
    await streamChat(
      { provider: 'mock', model: 'mock-1', messages: [{ role: 'user', content: 'hi' }] },
      { onDelta: (d) => deltas.push(d), onError: (e) => (error = e) },
    );

    expect(deltas.join('')).toBe('Hello');
    expect(error).toBeNull();
  });

  it('reports a stream error event', async () => {
    const sse = 'data: {"error":"upstream boom"}\n\n';
    vi.stubGlobal('fetch', vi.fn(async () => sseResponse(sse)));

    let error: string | null = null;
    await streamChat(
      { provider: 'mock', model: 'mock-1', messages: [{ role: 'user', content: 'hi' }] },
      { onDelta: () => {}, onError: (e) => (error = e) },
    );
    expect(error).toBe('upstream boom');
  });
});
