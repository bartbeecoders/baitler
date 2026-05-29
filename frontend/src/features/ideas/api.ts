import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { ApiError, apiFetch } from '@/lib/api';
import type { Idea, IdeaDetail, IdeaFilters, IdeaStatus } from './types';

/** Best-effort human message from a thrown API/network error. */
export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const IDEAS_ROOT = ['ideas'] as const;
export const ideasKey = (filters: IdeaFilters) => ['ideas', 'list', filters] as const;
export const ideaKey = (id: string) => ['ideas', 'detail', id] as const;
export const tagsKey = ['ideas', 'tags'] as const;

function jsonInit(method: string, body: unknown): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  };
}

export function useIdeas(filters: IdeaFilters) {
  return useQuery({
    queryKey: ideasKey(filters),
    queryFn: () => {
      const params = new URLSearchParams();
      if (filters.status) params.set('status', filters.status);
      if (filters.tag) params.set('tag', filters.tag);
      if (filters.q) params.set('q', filters.q);
      const qs = params.toString();
      return apiFetch<{ ideas: Idea[] }>(`/ideas${qs ? `?${qs}` : ''}`).then((r) => r.ideas);
    },
  });
}

export function useIdea(id: string | null) {
  return useQuery({
    queryKey: ideaKey(id ?? ''),
    queryFn: () => apiFetch<IdeaDetail>(`/ideas/${id}`),
    enabled: !!id,
  });
}

export function useTags() {
  return useQuery({
    queryKey: tagsKey,
    queryFn: () => apiFetch<{ tags: string[] }>('/ideas/tags').then((r) => r.tags),
  });
}

interface IdeaInput {
  title: string;
  body: string;
  tags: string[];
  status: IdeaStatus;
}

export function useCreateIdea() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: IdeaInput) => apiFetch<Idea>('/ideas', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: IDEAS_ROOT }),
  });
}

export function useUpdateIdea() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, patch }: { id: string; patch: Partial<IdeaInput> }) =>
      apiFetch<Idea>(`/ideas/${id}`, jsonInit('PATCH', patch)),
    onSuccess: () => qc.invalidateQueries({ queryKey: IDEAS_ROOT }),
  });
}

export function useDeleteIdea() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/ideas/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: IDEAS_ROOT }),
  });
}

export function useLinkIdea() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, targetId }: { id: string; targetId: string }) =>
      apiFetch(`/ideas/${id}/links`, jsonInit('POST', { target_id: targetId })),
    onSuccess: () => qc.invalidateQueries({ queryKey: IDEAS_ROOT }),
  });
}

export function useUnlinkIdea() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, targetId }: { id: string; targetId: string }) =>
      apiFetch(`/ideas/${id}/links/${targetId}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: IDEAS_ROOT }),
  });
}
