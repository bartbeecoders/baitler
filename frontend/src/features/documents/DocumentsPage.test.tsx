import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { DocumentsPage } from './DocumentsPage';

const DOC = {
  id: 'd1',
  title: 'Quarterly memo',
  body: '<p>hi</p>',
  version: 2,
  tags: [],
  created_at: '2026-06-01T00:00:00Z',
  updated_at: '2026-06-01T00:00:00Z',
};

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/documents/d1') return DOC;
    return {};
  }),
  apiFetchBlob: vi.fn(),
}));

// The list (and its New control) now lives in the sidebar; the page renders only
// the editor for the routed id. Stub the heavy TipTap editor.
vi.mock('./RichTextEditor', () => ({ RichTextEditor: () => <div data-testid="rich-text-editor" /> }));

function renderAt(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/editor" element={<DocumentsPage />} />
          <Route path="/editor/:id" element={<DocumentsPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('DocumentsPage (editor route)', () => {
  it('shows the empty hint when no document is selected', () => {
    renderAt('/editor');
    expect(screen.getByText(/select a document/i)).toBeInTheDocument();
  });

  it('renders the editor for the routed document', async () => {
    renderAt('/editor/d1');
    await waitFor(() =>
      expect(screen.getByLabelText('Document title')).toHaveValue('Quarterly memo'),
    );
  });
});
