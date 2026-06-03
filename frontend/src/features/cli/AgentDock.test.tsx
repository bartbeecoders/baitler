import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { AgentDock } from './AgentDock';
import { pageContext } from './context';

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/ai/providers') return { providers: [] };
    if (path === '/projects') return { projects: [] };
    if (path === '/cli/runs') return { runs: [] };
    if (path === '/cli/status')
      return {
        enabled: true,
        kind: 'claude-cli',
        binary_ok: true,
        version: '1',
        has_stored_key: true,
        host_key_env: false,
        ready: true,
        message: 'Ready.',
        providers: [{ id: 'claude_code', label: 'Claude Code', available: true, detail: 'ok' }],
      };
    return null;
  }),
}));

describe('pageContext', () => {
  it('maps routes to a page label + orienting context', () => {
    expect(pageContext('/files').label).toBe('Files');
    expect(pageContext('/files/123').label).toBe('Files');
    expect(pageContext('/editor').label).toBe('Documents');
    expect(pageContext('/pages').label).toBe('Pages');
    expect(pageContext('/').label).toBe('Dashboard');

    const ctx = pageContext('/ideas').context;
    expect(ctx).toContain('Ideas');
    expect(ctx).toMatch(/draft/i);
  });
});

describe('AgentDock', () => {
  it('renders the embedded panel labelled with the current page', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/ideas']}>
          <AgentDock />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(screen.getByRole('complementary', { name: 'Agent' })).toBeInTheDocument();
    expect(screen.getByText('Ideas')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Task' })).toBeInTheDocument();
    // Backdrop + the explicit X both close the pane.
    expect(screen.getAllByRole('button', { name: /close agent/i }).length).toBeGreaterThan(0);
  });
});
