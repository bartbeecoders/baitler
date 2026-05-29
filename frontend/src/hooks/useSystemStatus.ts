import { useQuery, type UseQueryResult } from '@tanstack/react-query';

import { apiFetch } from '@/lib/api';
import type { HealthResponse, VersionResponse } from '@/types/api';

/** Polls the backend readiness probe; drives the live API status indicator. */
export function useHealth() {
  return useQuery({
    queryKey: ['health'],
    queryFn: ({ signal }) => apiFetch<HealthResponse>('/health', { signal }),
    refetchInterval: 30_000,
    retry: 0,
  });
}

/** Fetches backend build metadata (name/version). */
export function useVersion() {
  return useQuery({
    queryKey: ['version'],
    queryFn: ({ signal }) => apiFetch<VersionResponse>('/version', { signal }),
    retry: 0,
  });
}

export type ApiStatus = 'loading' | 'connected' | 'offline';

/**
 * Derive a single status from a health query result. Shared by the header badge
 * and the dashboard panel so the "is the API up?" rule lives in one place.
 */
export function apiStatusOf(query: UseQueryResult<HealthResponse>): ApiStatus {
  if (query.isLoading) return 'loading';
  if (query.isError || query.data?.status !== 'ok') return 'offline';
  return 'connected';
}
