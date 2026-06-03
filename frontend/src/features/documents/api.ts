import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { API_BASE_URL, ApiError, apiFetch } from '@/lib/api';
import type { Document, DocumentSummary, ExportFormat, ExportSource } from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const DOCS_ROOT = ['documents'] as const;
export const docKey = (id: string) => ['documents', 'detail', id] as const;

export function useDocuments() {
  return useQuery({
    queryKey: ['documents', 'list'],
    queryFn: () => apiFetch<{ documents: DocumentSummary[] }>('/documents').then((r) => r.documents),
  });
}

export function useDocument(id: string | null) {
  return useQuery({
    queryKey: docKey(id ?? ''),
    queryFn: () => apiFetch<Document>(`/documents/${id}`),
    enabled: !!id,
  });
}

function jsonInit(method: string, body: unknown): RequestInit {
  return { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
}

export function useCreateDocument() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { title: string; body?: string; tags?: string[] }) =>
      apiFetch<Document>('/documents', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: DOCS_ROOT }),
  });
}

export function useUpdateDocument() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      patch,
    }: {
      id: string;
      patch: { title?: string; body?: string; tags?: string[] };
    }) => apiFetch<Document>(`/documents/${id}`, jsonInit('PATCH', patch)),
    onSuccess: () => qc.invalidateQueries({ queryKey: DOCS_ROOT }),
  });
}

export function useDeleteDocument() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/documents/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: DOCS_ROOT }),
  });
}

/** Convert + download via the shared `POST /export` pathway. */
export async function exportDownload(req: {
  content: string;
  source: ExportSource;
  target: ExportFormat;
  filename: string;
}): Promise<void> {
  const res = await fetch(`${API_BASE_URL}/export`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  });
  if (!res.ok) {
    let message = res.statusText || 'Export failed';
    try {
      const body = await res.json();
      if (body?.error?.message) message = body.error.message;
    } catch {
      /* non-JSON error */
    }
    throw new Error(message);
  }
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  try {
    const ext = req.target === 'markdown' ? 'md' : req.target;
    const a = document.createElement('a');
    a.href = url;
    a.download = `${req.filename || 'export'}.${ext}`;
    document.body.appendChild(a);
    a.click();
    a.remove();
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Convert Markdown → HTML via the export pathway (used for `.md` import). */
export async function markdownToHtml(markdown: string): Promise<string> {
  const res = await fetch(`${API_BASE_URL}/export`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content: markdown, source: 'markdown', target: 'html' }),
  });
  if (!res.ok) throw new Error('Failed to import Markdown');
  return res.text();
}
