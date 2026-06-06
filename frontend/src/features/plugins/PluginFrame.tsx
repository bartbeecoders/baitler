import { useCallback, useEffect, useRef } from 'react';

import { API_BASE_URL, apiFetch } from '@/lib/api';
import { useThemeStore } from '@/stores/theme';
import type { PluginUiMount } from './types';

/**
 * The host side of the plugin-UI bridge.
 *
 * The component renders an `<iframe sandbox="allow-scripts">` onto the
 * backend's opaque-origin asset route (`/plugin-ui/{uuid}/{entry}`); the
 * served CSP additionally forces `sandbox` without `allow-same-origin` and
 * `connect-src 'none'`, so plugin JS holds no credentials and cannot fetch —
 * its ONLY way to reach Baitler is this postMessage RPC, which the host
 * relays through the credentialed `apiFetch` and pins to the plugin's own
 * `/px/{slug}/…` endpoints:
 *
 * - host → frame on load: `{ baitler: 'init', slug, theme }`
 * - frame → host:         `{ baitler: 'fetch', id, path, method?, body? }`
 * - host → frame:         `{ baitler: 'result', id, ok, status?, body?, error? }`
 *
 * A sandboxed frame has an opaque origin (`event.origin === 'null'`), so the
 * trust check is the `event.source === contentWindow` identity, and replies
 * use the `'*'` target (an opaque origin cannot be named). Coarse, typed
 * messages only.
 */
export function PluginFrame({ mount, className }: { mount: PluginUiMount; className?: string }) {
  const frameRef = useRef<HTMLIFrameElement | null>(null);
  const theme = useThemeStore((s) => s.theme);
  const src = `${API_BASE_URL}/plugin-ui/${mount.pluginId}/${mount.entry}`;

  const post = useCallback((message: unknown) => {
    frameRef.current?.contentWindow?.postMessage(message, '*');
  }, []);

  useEffect(() => {
    const prefix = `/px/${mount.slug}/`;
    const apiOrigin = new URL(API_BASE_URL).origin;

    async function relay(data: {
      id: string;
      path: string;
      method?: string;
      body?: unknown;
    }) {
      const reject = (error: string) =>
        post({ baitler: 'result', id: data.id, ok: false, error });

      // Resolve against the API origin and compare the NORMALIZED pathname —
      // never the raw string. A raw `startsWith` check is defeated by path
      // traversal (`/px/{slug}/../../ai/keys` resolves to `/ai/keys` but still
      // literally starts with the prefix), which would let a sandboxed plugin
      // drive the credentialed client into ANY endpoint — including the
      // portal-only lifecycle verbs. The origin check also blocks absolute /
      // protocol-relative escapes.
      if (typeof data.path !== 'string') {
        reject('path must be a string');
        return;
      }
      let resolved: URL;
      try {
        resolved = new URL(data.path, API_BASE_URL);
      } catch {
        reject('path is not a valid URL');
        return;
      }
      if (resolved.origin !== apiOrigin || !resolved.pathname.startsWith(prefix)) {
        reject(`path must resolve under ${prefix} (a plugin reaches only its own endpoints)`);
        return;
      }
      const relayPath = resolved.pathname + resolved.search;
      try {
        const method = data.method ?? 'POST';
        const body = data.body === undefined ? undefined : JSON.stringify(data.body);
        const result = await apiFetch<unknown>(relayPath, {
          method,
          headers: body ? { 'Content-Type': 'application/json' } : undefined,
          body,
        });
        post({ baitler: 'result', id: data.id, ok: true, body: result });
      } catch (e) {
        reject(e instanceof Error ? e.message : 'request failed');
      }
    }

    function onMessage(event: MessageEvent) {
      // Identity, not origin: the sandboxed frame's origin is opaque ("null"),
      // so the only trustworthy check is that the sender IS our frame.
      if (!frameRef.current || event.source !== frameRef.current.contentWindow) return;
      const data = event.data as { baitler?: string; id?: string } | null;
      if (!data || data.baitler !== 'fetch' || typeof data.id !== 'string') return;
      void relay(data as { id: string; path: string; method?: string; body?: unknown });
    }

    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, [mount.slug, post]);

  return (
    <iframe
      ref={frameRef}
      src={src}
      title={mount.label}
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      className={className ?? 'h-full w-full rounded-md border border-border bg-background'}
      onLoad={() => post({ baitler: 'init', slug: mount.slug, theme })}
    />
  );
}
