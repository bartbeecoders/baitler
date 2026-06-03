import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { IdeasPage } from './IdeasPage';

const IDEA = {
  id: 'i1',
  title: 'Launch plan',
  body: 'ship it',
  tags: ['work'],
  status: 'active',
  links: [],
  related: [],
  created_at: '2026-05-29T00:00:00Z',
  updated_at: '2026-05-29T00:00:00Z',
};

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/ideas/i1') return IDEA;
    if (path.startsWith('/ideas/tags')) return { tags: ['work'] };
    return {};
  }),
  apiFetchBlob: vi.fn(),
}));

function renderAt(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/ideas" element={<IdeasPage />} />
          <Route path="/ideas/new" element={<IdeasPage />} />
          <Route path="/ideas/:id" element={<IdeasPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('IdeasPage (editor route)', () => {
  it('shows the empty hint and no open editor at /ideas', () => {
    renderAt('/ideas');
    expect(screen.getByText(/select a idea|select an idea/i)).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('opens the create editor at /ideas/new', () => {
    renderAt('/ideas/new');
    expect(screen.getByText('New idea')).toBeInTheDocument();
  });

  it('opens the edit editor for a routed idea', async () => {
    renderAt('/ideas/i1');
    expect(screen.getByText('Edit idea')).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByLabelText('Idea title')).toHaveValue('Launch plan'),
    );
  });
});
