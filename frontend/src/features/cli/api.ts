import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { API_BASE_URL, ApiError, apiFetch } from '@/lib/api';
import type { CliRun, CliRunSummary, CliStatus, CreateRunRequest, RunEvent } from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

export const CLI_ROOT = ['cli', 'runs'] as const;

export function useCliStatus() {
  return useQuery({
    queryKey: ['cli', 'status'],
    queryFn: () => apiFetch<CliStatus>('/cli/status'),
  });
}

export function useRuns() {
  return useQuery({
    queryKey: [...CLI_ROOT, 'list'],
    queryFn: () =>
      apiFetch<{ runs: CliRunSummary[] }>('/cli/runs').then((r) => r.runs),
  });
}

export function useRun(id: string | null) {
  return useQuery({
    queryKey: [...CLI_ROOT, 'detail', id ?? ''],
    queryFn: () => apiFetch<CliRun>(`/cli/runs/${id}`),
    enabled: !!id,
  });
}

export function useCancelRun() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/cli/runs/${id}/cancel`, { method: 'POST' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: CLI_ROOT }),
  });
}

/** Minimal project list for the run's target-project picker. */
export function useProjectOptions() {
  return useQuery({
    queryKey: ['projects', 'list'],
    queryFn: () =>
      apiFetch<{ projects: { id: string; name: string }[] }>('/projects').then((r) => r.projects),
  });
}

export interface RunStreamHandlers {
  onEvent: (event: RunEvent) => void;
  /** `status` is the HTTP status when the request itself failed (503/409/…). */
  onError: (message: string, status?: number) => void;
  signal?: AbortSignal;
}

/**
 * POST /cli/runs and stream the SSE response, invoking `onEvent` per run event.
 * Resolves when the stream finishes (or is aborted). Connection-time failures
 * (disabled runner → 503, concurrent run → 409, validation → 400) surface via
 * `onError` with the HTTP status.
 */
export async function streamRun(req: CreateRunRequest, handlers: RunStreamHandlers): Promise<void> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE_URL}/cli/runs`, {
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
    handlers.onError(message, res.status);
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

      let sep: number;
      while ((sep = buffer.indexOf('\n\n')) >= 0) {
        const event = buffer.slice(0, sep);
        buffer = buffer.slice(sep + 2);
        const dataLine = event.split('\n').find((l) => l.startsWith('data:'));
        if (!dataLine) continue;
        try {
          const json = JSON.parse(dataLine.slice(5).trim()) as RunEvent;
          if (json && typeof (json as { type?: unknown }).type === 'string') {
            handlers.onEvent(json);
          }
        } catch {
          /* ignore malformed event */
        }
      }
    }
  } catch {
    // Aborts surface here; treat as a normal stop.
  }
}

export type SaveTarget = 'idea' | 'document' | 'page';

/**
 * Save a run's result text as a new draft artifact, reusing the existing feature
 * endpoints. Ideas/documents are created then flipped to `review="draft"` (their
 * portal default is published); pages default to `visibility="draft"`.
 */
export async function saveResultAs(
  target: SaveTarget,
  input: { title: string; body: string },
): Promise<void> {
  if (target === 'page') {
    await apiFetch('/pages', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ title: input.title, body: input.body, source_format: 'markdown' }),
    });
    return;
  }

  const path = target === 'idea' ? '/ideas' : '/documents';
  const created = await apiFetch<{ id: string }>(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ title: input.title, body: input.body }),
  });
  if (created?.id) {
    await apiFetch(`${path}/${created.id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ review: 'draft' }),
    });
  }
}
