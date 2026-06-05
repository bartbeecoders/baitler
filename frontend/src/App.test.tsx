import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';

import App from './App';

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/health') return { status: 'ok', db: 'up' };
    if (path === '/version') return { name: 'baitler-api', version: '0.1.0', git_sha: null };
    if (path.startsWith('/files')) {
      return { folder: null, breadcrumbs: [], folders: [], files: [] };
    }
    // The butler home's readiness/feed queries.
    if (path === '/cli/status')
      return {
        enabled: true,
        kind: 'mock',
        binary_ok: true,
        version: null,
        has_stored_key: false,
        host_key_env: false,
        ready: true,
        message: 'Ready.',
        providers: [{ id: 'claude_code', label: 'Claude Code', available: true, detail: 'ok' }],
        workspace_roots: [],
      };
    if (path === '/cli/runs') return { runs: [] };
    if (path === '/ai/providers') return { providers: [] };
    if (path === '/projects') return { projects: [] };
    if (path === '/review') return { ideas: [], documents: [] };
    throw new Error(`unexpected path: ${path}`);
  }),
  apiFetchBlob: vi.fn(async () => new Blob()),
}));

function renderAt(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('App routing', () => {
  it('renders the butler home at / (lazy-loaded)', async () => {
    renderAt('/');
    expect(
      await screen.findByRole('heading', { name: /what shall i organize for you/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('textbox', { name: /tell the butler what to do/i }),
    ).toBeInTheDocument();
  });

  it('renders the files page at /files (lazy-loaded)', async () => {
    renderAt('/files');
    expect(await screen.findByRole('heading', { name: 'Files' })).toBeInTheDocument();
  });

  it('renders NotFound for an unknown route', () => {
    renderAt('/this-route-does-not-exist');
    expect(screen.getByRole('heading', { name: /page not found/i })).toBeInTheDocument();
    expect(screen.getByText('404')).toBeInTheDocument();
  });
});
