import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { ApiError, apiFetch } from '@/lib/api';
import type { Plugin, PluginUiMount, PluginUiSlot } from './types';

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof Error) return error.message;
  return 'Something went wrong';
}

const PLUGINS_ROOT = ['plugins'] as const;

export function usePlugins() {
  return useQuery({
    queryKey: ['plugins', 'list'],
    queryFn: () => apiFetch<{ plugins: Plugin[] }>('/plugins').then((r) => r.plugins),
  });
}

export function usePlugin(idOrSlug: string | null) {
  const list = usePlugins();
  const plugin =
    list.data?.find((p) => p.slug === idOrSlug || p.id === idOrSlug) ?? null;
  return { ...list, plugin };
}

/**
 * Enabled plugins, for the UI mount slots (rail / butler widget / objects /
 * detail). Chrome surfaces render on every page, so this never surfaces an
 * error — a disabled plugin system (503) or an offline backend simply mounts
 * nothing — and it refreshes lazily.
 */
export function useEnabledPlugins() {
  return useQuery({
    queryKey: ['plugins', 'enabled'],
    queryFn: () =>
      apiFetch<{ plugins: Plugin[] }>('/plugins?status=enabled').then((r) => r.plugins),
    retry: false,
    staleTime: 60_000,
  });
}

/** The enabled UI components declared for `slot`, flattened across plugins. */
export function uiMounts(plugins: Plugin[] | undefined, slot: PluginUiSlot): PluginUiMount[] {
  if (!plugins) return [];
  return plugins.flatMap((p) =>
    (p.manifest?.ui ?? [])
      .filter((u) => u.slot === slot)
      .map((u) => ({
        pluginId: p.id,
        slug: p.slug,
        pluginName: p.name,
        slot: u.slot,
        key: u.key,
        label: u.label || p.name,
        entry: u.entry,
      })),
  );
}

function lifecycleMutation(verb: 'approve' | 'enable' | 'disable' | 'reject') {
  return function useLifecycle() {
    const qc = useQueryClient();
    return useMutation({
      mutationFn: (id: string) =>
        apiFetch<Plugin>(`/plugins/${id}/${verb}`, { method: 'POST' }),
      onSuccess: () => qc.invalidateQueries({ queryKey: PLUGINS_ROOT }),
    });
  };
}

export const useApprovePlugin = lifecycleMutation('approve');
export const useEnablePlugin = lifecycleMutation('enable');
export const useDisablePlugin = lifecycleMutation('disable');
export const useRejectPlugin = lifecycleMutation('reject');

export function useUninstallPlugin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => apiFetch(`/plugins/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: PLUGINS_ROOT }),
  });
}
