import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { ApiError, apiFetch } from '@/lib/api';
import type { FolderItem } from '@/features/files/types';
import type { Diagram, DiagramListFilters, DiagramSummary } from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

/**
 * The origin the embedded draw.io editor is loaded from. Defaults to the hosted
 * embed; set `VITE_DRAWIO_URL` (e.g. a self-hosted deploy) for privacy. The live
 * editor frame loads ONLY on this authoring surface — never on `/p/*`.
 */
export const DRAWIO_URL: string =
  (import.meta.env.VITE_DRAWIO_URL as string | undefined)?.replace(/\/$/, '') ||
  'https://embed.diagrams.net';

const ROOT = ['diagrams'] as const;
export const diagramKey = (id: string) => ['diagrams', 'detail', id] as const;

export function useDiagrams(filters: DiagramListFilters) {
  return useQuery({
    queryKey: ['diagrams', 'list', filters],
    queryFn: () => {
      const params = new URLSearchParams();
      if (filters.q) params.set('q', filters.q);
      if (filters.folder) params.set('folder', filters.folder);
      const qs = params.toString();
      return apiFetch<{ diagrams: DiagramSummary[] }>(`/diagrams${qs ? `?${qs}` : ''}`).then(
        (r) => r.diagrams,
      );
    },
  });
}

export function useDiagram(id: string | null) {
  return useQuery({
    queryKey: diagramKey(id ?? ''),
    queryFn: () => apiFetch<Diagram>(`/diagrams/${id}`),
    enabled: !!id,
  });
}

export function useFolders() {
  return useQuery({
    queryKey: ['diagrams', 'folders'],
    queryFn: () => apiFetch<{ folders: FolderItem[] }>('/files').then((r) => r.folders ?? []),
  });
}

function jsonInit(method: string, body: unknown): RequestInit {
  return { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
}

export interface CreateDiagramInput {
  title: string;
  xml?: string;
  preview?: string;
  folder_id?: string;
}

export function useCreateDiagram() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateDiagramInput) =>
      apiFetch<Diagram>('/diagrams', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

export interface DiagramPatch {
  title?: string;
  xml?: string;
  preview?: string;
  folder_id?: string | null;
  tags?: string[];
}

export function useUpdateDiagram() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, patch }: { id: string; patch: DiagramPatch }) =>
      apiFetch<Diagram>(`/diagrams/${id}`, jsonInit('PATCH', patch)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

export function useDeleteDiagram() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/diagrams/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}
