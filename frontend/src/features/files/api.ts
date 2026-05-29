import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { ApiError, apiFetch, apiFetchBlob } from '@/lib/api';
import type { Listing } from './types';

export type ItemKind = 'file' | 'folder';

/** Best-effort human message from a thrown API/network error. */
export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

/** Query key for a directory listing / search. */
export const listingKey = (folderId: string | null, q: string) =>
  ['files', { folderId, q }] as const;

/** Fetch a file's bytes via the credentialed API path. */
export function fetchContent(id: string): Promise<Blob> {
  return apiFetchBlob(`/files/${id}/content`);
}

/** Download a file by streaming its bytes to a transient object URL. */
export async function downloadContent(id: string, name: string): Promise<void> {
  const blob = await fetchContent(id);
  const url = URL.createObjectURL(blob);
  try {
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
  } finally {
    URL.revokeObjectURL(url);
  }
}

export function useDownload() {
  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) => downloadContent(id, name),
  });
}

export function useListing(folderId: string | null, q: string) {
  return useQuery({
    queryKey: listingKey(folderId, q),
    queryFn: () => {
      const params = new URLSearchParams();
      if (q) params.set('q', q);
      else if (folderId) params.set('folder', folderId);
      const qs = params.toString();
      return apiFetch<Listing>(`/files${qs ? `?${qs}` : ''}`);
    },
  });
}

function useInvalidateFiles() {
  const queryClient = useQueryClient();
  return () => queryClient.invalidateQueries({ queryKey: ['files'] });
}

export function useUpload() {
  const invalidate = useInvalidateFiles();
  return useMutation({
    mutationFn: ({ files, folderId }: { files: File[]; folderId: string | null }) => {
      const form = new FormData();
      for (const file of files) form.append('file', file);
      const qs = folderId ? `?folder=${encodeURIComponent(folderId)}` : '';
      return apiFetch(`/files${qs}`, { method: 'POST', body: form });
    },
    onSuccess: invalidate,
  });
}

export function useCreateFolder() {
  const invalidate = useInvalidateFiles();
  return useMutation({
    mutationFn: ({ name, parentId }: { name: string; parentId: string | null }) =>
      apiFetch('/folders', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, parent_id: parentId }),
      }),
    onSuccess: invalidate,
  });
}

export function useRename() {
  const invalidate = useInvalidateFiles();
  return useMutation({
    mutationFn: ({ kind, id, name }: { kind: ItemKind; id: string; name: string }) =>
      apiFetch(`/${kind === 'file' ? 'files' : 'folders'}/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      }),
    onSuccess: invalidate,
  });
}

export function useDeleteItem() {
  const invalidate = useInvalidateFiles();
  return useMutation({
    mutationFn: ({ kind, id }: { kind: ItemKind; id: string }) =>
      apiFetch(`/${kind === 'file' ? 'files' : 'folders'}/${id}`, { method: 'DELETE' }),
    onSuccess: invalidate,
  });
}
