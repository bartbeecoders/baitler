import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';

import { PluginFrame } from './PluginFrame';
import type { PluginUiMount } from './types';

const apiFetch = vi.fn(async (_path: string, _init?: RequestInit) => ({ ok: true }));

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: (path: string, init?: RequestInit) => apiFetch(path, init),
}));

const MOUNT: PluginUiMount = {
  pluginId: 'uuid-1',
  slug: 'csv-stats',
  pluginName: 'CSV Stats',
  slot: 'detail',
  key: 'main',
  label: 'CSV Stats',
  entry: 'ui/index.html',
};

function frameOf(container: HTMLElement): HTMLIFrameElement {
  const frame = container.querySelector('iframe');
  if (!frame) throw new Error('no iframe rendered');
  return frame;
}

/** Dispatch a bridge message as if it came from `source`. */
function postFromFrame(source: Window | null, data: unknown) {
  window.dispatchEvent(new MessageEvent('message', { data, source }));
}

describe('PluginFrame', () => {
  beforeEach(() => apiFetch.mockClear());

  it('mounts the sandboxed iframe on the opaque-origin asset route', () => {
    const { container } = render(<PluginFrame mount={MOUNT} />);
    const frame = frameOf(container);
    expect(frame.src).toBe('http://localhost:8080/plugin-ui/uuid-1/ui/index.html');
    expect(frame.getAttribute('sandbox')).toBe('allow-scripts');
    expect(frame.getAttribute('referrerpolicy')).toBe('no-referrer');
  });

  it('relays a fetch message from its own frame to the plugin endpoints', async () => {
    const { container } = render(<PluginFrame mount={MOUNT} />);
    const frame = frameOf(container);
    postFromFrame(frame.contentWindow, {
      baitler: 'fetch',
      id: 'r1',
      path: '/px/csv-stats/summarize',
      method: 'POST',
      body: { a: 1 },
    });
    await waitFor(() => expect(apiFetch).toHaveBeenCalledTimes(1));
    expect(apiFetch).toHaveBeenCalledWith(
      '/px/csv-stats/summarize',
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('ignores messages from any other source window', async () => {
    render(<PluginFrame mount={MOUNT} />);
    postFromFrame(window, {
      baitler: 'fetch',
      id: 'r2',
      path: '/px/csv-stats/summarize',
    });
    postFromFrame(null, {
      baitler: 'fetch',
      id: 'r3',
      path: '/px/csv-stats/summarize',
    });
    await new Promise((r) => setTimeout(r, 10));
    expect(apiFetch).not.toHaveBeenCalled();
  });

  it("refuses paths outside the plugin's own mount, incl. traversal escapes", async () => {
    const { container } = render(<PluginFrame mount={MOUNT} />);
    const frame = frameOf(container);
    const replies: unknown[] = [];
    if (frame.contentWindow) {
      vi.spyOn(frame.contentWindow, 'postMessage').mockImplementation((msg: unknown) => {
        replies.push(msg);
      });
    }
    const escapes = [
      '/plugins/uuid-1/enable', // a lifecycle verb — never relayed
      '/px/other-plugin/steal', // another plugin's mount
      '/px/csv-stats/../../ai/keys', // traversal → /ai/keys
      '/px/csv-stats/sub/../../../plugins/uuid-1/enable', // traversal → lifecycle
      'http://evil.example/px/csv-stats/x', // absolute origin escape
      '//evil.example/px/csv-stats/x', // protocol-relative escape
    ];
    escapes.forEach((path, i) =>
      postFromFrame(frame.contentWindow, { baitler: 'fetch', id: `e${i}`, path }),
    );
    await waitFor(() => expect(replies.length).toBe(escapes.length));
    // The decisive assertion: NONE of these reached the credentialed client.
    expect(apiFetch).not.toHaveBeenCalled();
    for (const reply of replies as Array<{ ok: boolean }>) {
      expect(reply.ok).toBe(false);
    }
  });

  it('relays a legitimate in-mount path with a query string', async () => {
    const { container } = render(<PluginFrame mount={MOUNT} />);
    const frame = frameOf(container);
    postFromFrame(frame.contentWindow, {
      baitler: 'fetch',
      id: 'q1',
      path: '/px/csv-stats/report?days=7',
      method: 'GET',
    });
    await waitFor(() => expect(apiFetch).toHaveBeenCalledTimes(1));
    expect(apiFetch).toHaveBeenCalledWith(
      '/px/csv-stats/report?days=7',
      expect.objectContaining({ method: 'GET' }),
    );
  });
});
