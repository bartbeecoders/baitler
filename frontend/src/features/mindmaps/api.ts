import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { ApiError, apiFetch } from '@/lib/api';
import type { FolderItem } from '@/features/files/types';
import type { Project } from '@/features/projects/types';
import type {
  Mindmap,
  MindmapGraph,
  MindmapListFilters,
  MindmapSummary,
} from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const ROOT = ['mindmaps'] as const;
export const mindmapKey = (id: string) => ['mindmaps', 'detail', id] as const;

export function useMindmaps(filters: MindmapListFilters) {
  return useQuery({
    queryKey: ['mindmaps', 'list', filters],
    queryFn: () => {
      const params = new URLSearchParams();
      if (filters.q) params.set('q', filters.q);
      if (filters.folder) params.set('folder', filters.folder);
      const qs = params.toString();
      return apiFetch<{ mindmaps: MindmapSummary[] }>(`/mindmaps${qs ? `?${qs}` : ''}`).then(
        (r) => r.mindmaps,
      );
    },
  });
}

export function useMindmap(id: string | null) {
  return useQuery({
    queryKey: mindmapKey(id ?? ''),
    queryFn: () => apiFetch<Mindmap>(`/mindmaps/${id}`),
    enabled: !!id,
  });
}

/** Top-level folders, for the folder filter and the folder picker. */
export function useFolders() {
  return useQuery({
    queryKey: ['mindmaps', 'folders'],
    queryFn: () => apiFetch<{ folders: FolderItem[] }>('/files').then((r) => r.folders ?? []),
  });
}

/** Projects, for the "seed from project" picker. */
export function useProjects() {
  return useQuery({
    queryKey: ['mindmaps', 'projects'],
    queryFn: () => apiFetch<{ projects: Project[] }>('/projects').then((r) => r.projects ?? []),
  });
}

function jsonInit(method: string, body: unknown): RequestInit {
  return { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
}

export interface CreateMindmapInput {
  title: string;
  graph?: MindmapGraph;
  outline?: string;
  folder_id?: string;
}

export function useCreateMindmap() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateMindmapInput) =>
      apiFetch<Mindmap>('/mindmaps', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

export interface MindmapPatch {
  title?: string;
  graph?: MindmapGraph;
  folder_id?: string | null;
  tags?: string[];
}

export function useUpdateMindmap() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, patch }: { id: string; patch: MindmapPatch }) =>
      apiFetch<Mindmap>(`/mindmaps/${id}`, jsonInit('PATCH', patch)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

export function useDeleteMindmap() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/mindmaps/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}

/** Seed a new mindmap from a project's ideas + cross-links. */
export function useSeedFromProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (project_id: string) =>
      apiFetch<Mindmap>('/mindmaps/from-project', jsonInit('POST', { project_id })),
    onSuccess: () => qc.invalidateQueries({ queryKey: ROOT }),
  });
}
