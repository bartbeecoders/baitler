import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { ApiError, apiFetch } from '@/lib/api';
import type { Project } from '@/features/projects/types';
import type {
  Superpage,
  SuperpageContextBlock,
  SuperpageLayout,
  SuperpageListFilters,
  SuperpageSummary,
} from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const ROOT = ['superpages'] as const;
export const superpageKey = (id: string) => ['superpages', 'detail', id] as const;

export function useSuperpages(filters: SuperpageListFilters) {
  return useQuery({
    queryKey: ['superpages', 'list', filters],
    queryFn: () => {
      const params = new URLSearchParams();
      if (filters.q) params.set('q', filters.q);
      if (filters.folder) params.set('folder', filters.folder);
      if (filters.project) params.set('project', filters.project);
      const qs = params.toString();
      return apiFetch<{ superpages: SuperpageSummary[] }>(
        `/superpages${qs ? `?${qs}` : ''}`,
      ).then((r) => r.superpages);
    },
  });
}

export function useSuperpage(id: string | null) {
  return useQuery({
    queryKey: superpageKey(id ?? ''),
    queryFn: () => apiFetch<Superpage>(`/superpages/${id}`),
    enabled: !!id,
  });
}

/**
 * Resolve every block into a renderable preview (idea/document/page bodies,
 * diagram thumbnails, mindmap graphs, file metadata). Keyed on `version` so a
 * save (which bumps the version) transparently refreshes the previews.
 */
export function useSuperpageContext(id: string | null, version?: number) {
  return useQuery({
    queryKey: ['superpages', 'context', id, version],
    queryFn: () =>
      apiFetch<{ blocks: SuperpageContextBlock[] }>(`/superpages/${id}/context`).then(
        (r) => r.blocks,
      ),
    enabled: !!id,
  });
}

export function useProjects() {
  return useQuery({
    queryKey: ['superpages', 'projects'],
    queryFn: () => apiFetch<{ projects: Project[] }>('/projects').then((r) => r.projects ?? []),
  });
}

function jsonInit(method: string, body: unknown): RequestInit {
  return { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
}

export function useCreateSuperpage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { title: string; blocks?: SuperpageLayout }) =>
      apiFetch<Superpage>('/superpages', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

export function useCreateSuperpageFromProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { project_id: string; title?: string }) =>
      apiFetch<Superpage>('/superpages/from-project', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

export function useUpdateSuperpage(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { title?: string; blocks?: SuperpageLayout; tags?: string[] }) =>
      apiFetch<Superpage>(`/superpages/${id}`, jsonInit('PATCH', input)),
    onSuccess: (data) => {
      qc.setQueryData(superpageKey(id), data);
      qc.invalidateQueries({ queryKey: ROOT });
    },
  });
}

export function useDeleteSuperpage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch<void>(`/superpages/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

export interface GeneratedImage {
  /** A renderable `data:` URL (or remote URL) for the generated image. */
  data_url: string;
  mime: string;
  provider: string;
}

/** Generate an image for an image part (offline `mock` provider by default). */
export function useGenerateImage() {
  return useMutation({
    mutationFn: (input: { prompt: string; provider?: string }) =>
      apiFetch<GeneratedImage>('/ai/image', jsonInit('POST', input)),
  });
}