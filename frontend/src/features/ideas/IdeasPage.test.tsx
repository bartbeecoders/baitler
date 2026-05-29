import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { IdeasPage } from './IdeasPage';

const ideas = [
  {
    id: 'i1',
    title: 'Launch plan',
    body: 'ship it',
    tags: ['work'],
    status: 'active',
    links: [],
    created_at: '2026-05-29T00:00:00Z',
    updated_at: '2026-05-29T00:00:00Z',
  },
  {
    id: 'i2',
    title: 'Reading list',
    body: '',
    tags: ['personal'],
    status: 'inbox',
    links: [],
    created_at: '2026-05-29T00:00:00Z',
    updated_at: '2026-05-29T00:00:00Z',
  },
];

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path.startsWith('/ideas/tags')) return { tags: ['work', 'personal'] };
    if (path === '/ideas' || path.startsWith('/ideas?')) return { ideas };
    throw new Error(`unexpected path: ${path}`);
  }),
  apiFetchBlob: vi.fn(),
}));

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <IdeasPage />
    </QueryClientProvider>,
  );
}

describe('IdeasPage', () => {
  it('renders ideas from the list', async () => {
    renderPage();
    expect(screen.getByRole('heading', { name: 'Ideas' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('Launch plan')).toBeInTheDocument());
    expect(screen.getByText('Reading list')).toBeInTheDocument();
  });

  it('switches to board view showing status columns', async () => {
    renderPage();
    await waitFor(() => expect(screen.getByText('Launch plan')).toBeInTheDocument());
    await userEvent.click(screen.getByRole('button', { name: /board view/i }));
    // Status column headings (with counts) appear in board view.
    expect(screen.getByRole('heading', { name: /inbox/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /active/i })).toBeInTheDocument();
  });

  it('opens the new-idea editor', async () => {
    renderPage();
    await userEvent.click(screen.getByRole('button', { name: /new idea/i }));
    expect(screen.getByRole('dialog', { name: 'New idea' })).toBeInTheDocument();
  });
});
