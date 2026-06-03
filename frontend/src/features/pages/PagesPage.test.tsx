import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { PagesPage } from './PagesPage';

const PUBLISHED_PAGE = {
  id: 'p1',
  title: 'Landing',
  body: '<h1>Hello</h1>',
  slug: 'landing',
  visibility: 'public',
  source_format: 'html',
  folder_id: null,
  project_id: null,
  version: 2,
  published_at: '2026-06-01T00:00:00Z',
  tags: [],
  public_url: '/p/landing',
  created_at: '2026-06-01T00:00:00Z',
  updated_at: '2026-06-01T00:00:00Z',
};

const apiFetch = vi.fn(async (path: string, init?: RequestInit) => {
  if (path === '/pages/p1') return PUBLISHED_PAGE;
  if (path === '/files') return { folders: [] };
  if (path === '/pages/p1/unpublish') return { ...PUBLISHED_PAGE, visibility: 'draft', public_url: '' };
  if (path.startsWith('/pages/p1/publish')) return PUBLISHED_PAGE;
  if (init?.method === 'PATCH') return PUBLISHED_PAGE;
  return {};
});

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: (path: string, init?: RequestInit) => apiFetch(path, init),
}));

// The page now renders only the editor for the routed page; the list lives in the sidebar.
vi.mock('@/features/documents/RichTextEditor', () => ({
  RichTextEditor: () => <div data-testid="rich-text-editor" />,
}));

function renderAt(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/pages" element={<PagesPage />} />
          <Route path="/pages/:id" element={<PagesPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('PagesPage (editor route)', () => {
  beforeEach(() => {
    apiFetch.mockClear();
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it('shows the empty hint when no page is selected', () => {
    renderAt('/pages');
    expect(screen.getByText(/select a page/i)).toBeInTheDocument();
  });

  it('publishes/unpublishes via the visibility toggle', async () => {
    renderAt('/pages/p1');
    const select = await screen.findByLabelText('Page visibility');
    expect((select as HTMLSelectElement).value).toBe('public');
    fireEvent.change(select, { target: { value: 'draft' } });
    await waitFor(() =>
      expect(apiFetch).toHaveBeenCalledWith('/pages/p1/unpublish', expect.objectContaining({ method: 'POST' })),
    );
  });

  it('copies the share link to the clipboard', async () => {
    renderAt('/pages/p1');
    const copy = await screen.findByRole('button', { name: /copy share link/i });
    fireEvent.click(copy);
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('http://localhost:8080/p/landing'),
    );
  });

  it('previews a published page only in a sandboxed iframe', async () => {
    renderAt('/pages/p1');
    const previewBtn = await screen.findByRole('button', { name: /^preview$/i });
    fireEvent.click(previewBtn);
    const iframe = await screen.findByTitle('Page preview');
    expect(iframe).toHaveAttribute('sandbox', '');
    expect(iframe).toHaveAttribute('src', 'http://localhost:8080/p/landing');
  });
});
