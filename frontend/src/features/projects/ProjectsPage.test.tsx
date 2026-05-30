import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';

import { ProjectsPage } from './ProjectsPage';

const projects = [
  {
    id: 'p1',
    name: 'Quarterly Report',
    slug: 'quarterly-report',
    summary: 'Q3 numbers',
    status: 'active',
    created_at: '2026-05-29T00:00:00Z',
    updated_at: '2026-05-29T00:00:00Z',
  },
];

const reviewQueue = {
  ideas: [{ id: 'i1', title: 'Draft idea', review: 'draft' }],
  documents: [{ id: 'd1', title: 'Draft doc', review: 'draft' }],
};

const activity = [
  {
    id: 'a1',
    agent: 'claude-code',
    action: 'document.create',
    target_type: 'document',
    target_id: 'd1',
    target_title: 'Draft doc',
    project_id: 'p1',
    summary: 'document.create · Draft doc',
    created_at: '2026-05-29T00:00:00Z',
  },
];

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/projects') return { projects };
    if (path === '/review') return reviewQueue;
    if (path === '/activity') return { activity };
    throw new Error(`unexpected path: ${path}`);
  }),
  apiFetchBlob: vi.fn(),
}));

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('ProjectsPage', () => {
  it('lists projects and shows the pending-review count on the Review tab', async () => {
    renderPage();
    expect(screen.getByRole('heading', { name: 'Projects' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('Quarterly Report')).toBeInTheDocument());
    // 2 drafts (1 idea + 1 doc) → badge "2" next to the Review tab.
    await waitFor(() => expect(screen.getByText('2')).toBeInTheDocument());
  });

  it('shows draft items with Approve actions on the Review tab', async () => {
    renderPage();
    await userEvent.click(screen.getByRole('button', { name: /Review/i }));
    await waitFor(() => expect(screen.getByText('Draft doc')).toBeInTheDocument());
    expect(screen.getByText('Draft idea')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /Approve/i })).toHaveLength(2);
  });

  it('shows attributed activity on the Activity tab', async () => {
    renderPage();
    await userEvent.click(screen.getByRole('button', { name: /Activity/i }));
    await waitFor(() => expect(screen.getByText('document.create')).toBeInTheDocument());
    expect(screen.getByText('claude-code')).toBeInTheDocument();
  });
});
