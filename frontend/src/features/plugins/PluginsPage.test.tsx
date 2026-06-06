import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { PluginsPage } from './PluginsPage';

const DRAFT_PLUGIN = {
  id: 'pl1',
  name: 'CSV Stats',
  slug: 'csv-stats',
  description: 'Summarizes CSV data',
  version: '0.1.0',
  version_int: 1,
  api_version: 1,
  manifest: {
    name: 'CSV Stats',
    tools: [{ name: 'summarize', export: 'summarize' }],
    ui: [{ slot: 'detail', key: 'main', label: 'CSV Stats', entry: 'ui/index.html' }],
    capabilities: { host_fns: ['kn_search'], memory_max_pages: 256, timeout_ms: 2000 },
  },
  kinds: ['mcp_tool', 'ui_component'],
  status: 'draft',
  review: 'draft',
  bundle_sha256: 'abcdef0123456789',
  project_id: null,
  author_agent: 'claude-code',
  run_id: 'run-12345678',
  created_at: '2026-06-06T00:00:00Z',
  updated_at: '2026-06-06T00:00:00Z',
};

const apiFetch = vi.fn(async (path: string, init?: RequestInit) => {
  if (path === '/plugins' && !init?.method) return { plugins: [DRAFT_PLUGIN] };
  if (path === '/plugins/pl1/approve') {
    return { ...DRAFT_PLUGIN, status: 'approved', review: 'published' };
  }
  return {};
});

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: (path: string, init?: RequestInit) => apiFetch(path, init),
}));

function renderPage(initialPath = '/plugins') {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[initialPath]}>
        <Routes>
          <Route path="/plugins" element={<PluginsPage />} />
          <Route path="/plugins/:id" element={<PluginsPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('PluginsPage', () => {
  beforeEach(() => apiFetch.mockClear());

  it('shows a draft awaiting review with its permission diff and provenance', async () => {
    renderPage();
    expect(await screen.findByText('CSV Stats')).toBeInTheDocument();
    expect(screen.getByText(/Awaiting review \(1\)/)).toBeInTheDocument();
    // The capability the human is approving is visible…
    expect(screen.getByText('kn_search')).toBeInTheDocument();
    // …and so is who authored it.
    expect(screen.getByText(/authored by claude-code/)).toBeInTheDocument();
  });

  it('approve hits the portal-only lifecycle endpoint', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Approve' }));
    await waitFor(() =>
      expect(apiFetch).toHaveBeenCalledWith(
        '/plugins/pl1/approve',
        expect.objectContaining({ method: 'POST' }),
      ),
    );
  });

  it('explains a non-enabled plugin on its detail route instead of framing it', async () => {
    renderPage('/plugins/csv-stats');
    expect(await screen.findByText(/is draft/)).toBeInTheDocument();
    expect(document.querySelector('iframe')).toBeNull();
  });
});
