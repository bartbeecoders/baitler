import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { API_BASE_URL, ApiError, apiFetch } from '@/lib/api';
import type { FolderItem } from '@/features/files/types';
import type { Page, PageListFilters, PageSummary, SourceFormat, Visibility } from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const PAGES_ROOT = ['pages'] as const;
export const pageKey = (id: string) => ['pages', 'detail', id] as const;

/** Make a (possibly origin-relative) `public_url` absolute for previews/links. */
export function absoluteUrl(publicUrl: string): string {
  if (!publicUrl) return '';
  return publicUrl.startsWith('http') ? publicUrl : `${API_BASE_URL}${publicUrl}`;
}

export function usePages(filters: PageListFilters) {
  return useQuery({
    queryKey: ['pages', 'list', filters],
    queryFn: () => {
      const params = new URLSearchParams();
      if (filters.q) params.set('q', filters.q);
      if (filters.visibility) params.set('visibility', filters.visibility);
      if (filters.folder) params.set('folder', filters.folder);
      const qs = params.toString();
      return apiFetch<{ pages: PageSummary[] }>(`/pages${qs ? `?${qs}` : ''}`).then((r) => r.pages);
    },
  });
}

export function usePage(id: string | null) {
  return useQuery({
    queryKey: pageKey(id ?? ''),
    queryFn: () => apiFetch<Page>(`/pages/${id}`),
    enabled: !!id,
  });
}

/** Top-level folders, for the folder filter and the folder picker. */
export function useFolders() {
  return useQuery({
    queryKey: ['pages', 'folders'],
    queryFn: () =>
      apiFetch<{ folders: FolderItem[] }>('/files').then((r) => r.folders ?? []),
  });
}

function jsonInit(method: string, body: unknown): RequestInit {
  return { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
}

export interface CreatePageInput {
  title: string;
  body?: string;
  source_format?: SourceFormat;
  folder_id?: string;
  tags?: string[];
}

export function useCreatePage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreatePageInput) => apiFetch<Page>('/pages', jsonInit('POST', input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: PAGES_ROOT }),
  });
}

export interface PagePatch {
  title?: string;
  body?: string;
  visibility?: Visibility;
  slug?: string;
  folder_id?: string | null;
  tags?: string[];
}

export function useUpdatePage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, patch }: { id: string; patch: PagePatch }) =>
      apiFetch<Page>(`/pages/${id}`, jsonInit('PATCH', patch)),
    onSuccess: () => qc.invalidateQueries({ queryKey: PAGES_ROOT }),
  });
}

export function useDeletePage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/pages/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: PAGES_ROOT }),
  });
}

/** Publish (→ unlisted|public) and return the page with its absolute `public_url`. */
export function usePublishPage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, visibility }: { id: string; visibility: Visibility }) =>
      apiFetch<Page>(`/pages/${id}/publish`, jsonInit('POST', { visibility })),
    onSuccess: () => qc.invalidateQueries({ queryKey: PAGES_ROOT }),
  });
}

/** Unpublish (→ draft); the page's URL immediately 404s. */
export function useUnpublishPage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch<Page>(`/pages/${id}/unpublish`, { method: 'POST' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: PAGES_ROOT }),
  });
}
