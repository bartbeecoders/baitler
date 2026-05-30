import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { ApiError, apiFetch } from '@/lib/api';
import type { Activity, ItemType, Project, ProjectDetail, ReviewQueue } from './types';

/** Best-effort human message from a thrown API/network error. */
export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const PROJECTS_ROOT = ['projects'] as const;
const REVIEW_KEY = ['review'] as const;
const ACTIVITY_KEY = ['activity'] as const;

function jsonInit(method: string, body: unknown): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  };
}

export function useProjects() {
  return useQuery({
    queryKey: [...PROJECTS_ROOT, 'list'],
    queryFn: () => apiFetch<{ projects: Project[] }>('/projects').then((r) => r.projects),
  });
}

export function useProject(id: string | null) {
  return useQuery({
    queryKey: [...PROJECTS_ROOT, 'detail', id ?? ''],
    queryFn: () => apiFetch<ProjectDetail>(`/projects/${id}`),
    enabled: !!id,
  });
}

export function useCreateProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { name: string; summary: string }) =>
      apiFetch<Project>('/projects', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: PROJECTS_ROOT }),
  });
}

export function useDeleteProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/projects/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: PROJECTS_ROOT }),
  });
}

export function useReviewQueue() {
  return useQuery({
    queryKey: REVIEW_KEY,
    queryFn: () => apiFetch<ReviewQueue>('/review'),
  });
}

/** Approve (publish) a draft idea or document via its feature endpoint. */
export function useApprove() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ type, id }: { type: 'idea' | 'document'; id: string }) => {
      const path = type === 'idea' ? `/ideas/${id}` : `/documents/${id}`;
      return apiFetch(path, jsonInit('PATCH', { review: 'published' }));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: REVIEW_KEY });
      qc.invalidateQueries({ queryKey: PROJECTS_ROOT });
    },
  });
}

export function useActivity() {
  return useQuery({
    queryKey: ACTIVITY_KEY,
    queryFn: () => apiFetch<{ activity: Activity[] }>('/activity').then((r) => r.activity),
  });
}

export type { ItemType };
