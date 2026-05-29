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
  it('renders the dashboard at /', () => {
    renderAt('/');
    expect(screen.getByRole('heading', { name: /welcome to baitler/i })).toBeInTheDocument();
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
