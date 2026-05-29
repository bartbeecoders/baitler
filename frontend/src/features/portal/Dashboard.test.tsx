import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';

import { Dashboard } from './Dashboard';

// Stub the API client so the dashboard's system-status queries resolve
// deterministically without touching the network.
vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/health') return { status: 'ok', db: 'up' };
    if (path === '/version') return { name: 'baitler-api', version: '0.1.0', git_sha: null };
    throw new Error(`unexpected path: ${path}`);
  }),
}));

function renderDashboard() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <Dashboard />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Dashboard', () => {
  it('renders the base-portal heading and feature cards', () => {
    renderDashboard();
    expect(screen.getByRole('heading', { name: /welcome to baitler/i })).toBeInTheDocument();
    for (const label of ['Files', 'Ideas', 'Documents', 'Analytics', 'AI']) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
  });

  it('links each feature card to its route', () => {
    renderDashboard();
    const hrefs = screen.getAllByRole('link').map((link) => link.getAttribute('href'));
    expect(hrefs).toEqual(
      expect.arrayContaining(['/files', '/ideas', '/editor', '/analytics', '/ai']),
    );
  });

  it('shows the backend version once the status query resolves', async () => {
    renderDashboard();
    expect(await screen.findByText(/baitler-api v0\.1\.0/)).toBeInTheDocument();
  });
});
