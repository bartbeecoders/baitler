import { afterEach, describe, expect, it, vi } from 'vitest';

import { ApiError, apiFetch } from './api';

function stubFetch(response: Response) {
  const fetchMock = vi.fn(async (_url: string, _init?: RequestInit) => response);
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}

describe('apiFetch', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('returns the typed body on a 200 JSON response', async () => {
    stubFetch(new Response(JSON.stringify({ status: 'ok' }), { status: 200 }));
    await expect(apiFetch('/health')).resolves.toEqual({ status: 'ok' });
  });

  it('wraps a network failure as ApiError(0, network_error)', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new TypeError('Failed to fetch');
      }),
    );
    await expect(apiFetch('/health')).rejects.toMatchObject({
      status: 0,
      code: 'network_error',
    });
  });

  it('maps the error envelope to ApiError(status, code, message)', async () => {
    stubFetch(
      new Response(JSON.stringify({ error: { code: 'not_found', message: 'missing' } }), {
        status: 404,
      }),
    );
    await expect(apiFetch('/x')).rejects.toMatchObject({
      status: 404,
      code: 'not_found',
      message: 'missing',
    });
  });

  it('falls back to http_error for a non-envelope error body', async () => {
    stubFetch(new Response('boom', { status: 500, statusText: 'Internal Server Error' }));
    const err = await apiFetch('/x').catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 500, code: 'http_error' });
  });

  it('throws invalid_response when a 2xx body is not valid JSON', async () => {
    stubFetch(new Response('<html>not json</html>', { status: 200 }));
    await expect(apiFetch('/x')).rejects.toMatchObject({
      status: 200,
      code: 'invalid_response',
    });
  });

  it('sends credentials and an Accept header', async () => {
    const fetchMock = stubFetch(new Response('{}', { status: 200 }));
    await apiFetch('/x');
    const init = fetchMock.mock.calls[0]?.[1];
    expect(init?.credentials).toBe('include');
    expect((init?.headers as Record<string, string>).Accept).toBe('application/json');
  });
});
