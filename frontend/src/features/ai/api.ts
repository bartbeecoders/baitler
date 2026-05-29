import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { API_BASE_URL, ApiError, apiFetch } from '@/lib/api';
import type { ChatMessage, Provider } from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const AI_ROOT = ['ai'] as const;

export function useProviders() {
  return useQuery({
    queryKey: ['ai', 'providers'],
    queryFn: () => apiFetch<{ providers: Provider[] }>('/ai/providers').then((r) => r.providers),
  });
}

export function useSetKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ provider, apiKey }: { provider: string; apiKey: string }) =>
      apiFetch(`/ai/keys/${provider}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ api_key: apiKey }),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: AI_ROOT }),
  });
}

export function useDeleteKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (provider: string) => apiFetch(`/ai/keys/${provider}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: AI_ROOT }),
  });
}

export interface ChatStreamRequest {
  provider: string;
  model: string;
  messages: ChatMessage[];
  context?: string;
}

export interface ChatStreamHandlers {
  onDelta: (text: string) => void;
  onError: (message: string) => void;
  signal?: AbortSignal;
}

/**
 * POST /ai/chat and stream the SSE response, invoking `onDelta` per text chunk.
 * Resolves when the stream finishes (or is aborted).
 */
export async function streamChat(req: ChatStreamRequest, handlers: ChatStreamHandlers): Promise<void> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE_URL}/ai/chat`, {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req),
      signal: handlers.signal,
    });
  } catch {
    handlers.onError('Could not reach the server');
    return;
  }

  if (!res.ok || !res.body) {
    let message = res.statusText || 'Request failed';
    try {
      const body = await res.json();
      if (body?.error?.message) message = body.error.message;
    } catch {
      /* non-JSON error */
    }
    handlers.onError(message);
    return;
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      // SSE events are separated by a blank line.
      let sep: number;
      while ((sep = buffer.indexOf('\n\n')) >= 0) {
        const event = buffer.slice(0, sep);
        buffer = buffer.slice(sep + 2);
        const dataLine = event.split('\n').find((l) => l.startsWith('data:'));
        if (!dataLine) continue;
        try {
          const json = JSON.parse(dataLine.slice(5).trim());
          if (typeof json.delta === 'string') handlers.onDelta(json.delta);
          else if (typeof json.error === 'string') handlers.onError(json.error);
        } catch {
          /* ignore malformed event */
        }
      }
    }
  } catch {
    // Aborts surface here; treat as a normal stop.
  }
}
