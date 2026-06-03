import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { DiagramsPage } from './DiagramsPage';

const DIAGRAM = {
  id: 'd1',
  title: 'Pipeline',
  xml: '<mxGraphModel></mxGraphModel>',
  preview: 'data:image/png;base64,AAAA',
  folder_id: null,
  project_id: null,
  tags: [],
  review: 'published',
  version: 1,
  published_at: null,
  created_at: '2026-06-01T00:00:00Z',
  updated_at: '2026-06-01T00:00:00Z',
};

const apiFetch = vi.fn(async (path: string, init?: RequestInit) => {
  if (path === '/diagrams/d1') return DIAGRAM;
  if (init?.method === 'PATCH') return DIAGRAM;
  return {};
});

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: (path: string, init?: RequestInit) => apiFetch(path, init),
}));

function renderAt(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/diagrams" element={<DiagramsPage />} />
          <Route path="/diagrams/:id" element={<DiagramsPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('DiagramsPage (editor route)', () => {
  beforeEach(() => apiFetch.mockClear());

  it('shows the empty hint when no diagram is selected', () => {
    renderAt('/diagrams');
    expect(screen.getByText(/select a diagram/i)).toBeInTheDocument();
  });

  it('opens the draw.io editor by default with full UI params', async () => {
    renderAt('/diagrams/d1');
    const iframe = await screen.findByTitle('draw.io editor');
    expect(iframe.getAttribute('src')).toContain('embed=1');
    expect(iframe.getAttribute('src')).toContain('proto=json');
    expect(iframe.getAttribute('src')).toContain('ui=kennedy');
    expect(iframe.getAttribute('src')).toContain('libraries=1');
    expect(iframe.getAttribute('src')).toContain('embed.diagrams.net');
    expect(screen.queryByRole('img', { name: 'Pipeline' })).not.toBeInTheDocument();
  });

  it('shows a static preview when Preview is toggled off', async () => {
    renderAt('/diagrams/d1');
    await screen.findByTitle('draw.io editor');
    fireEvent.click(screen.getByRole('button', { name: /^preview$/i }));
    await waitFor(() => expect(screen.getByRole('img', { name: 'Pipeline' })).toBeInTheDocument());
    expect(screen.queryByTitle('draw.io editor')).not.toBeInTheDocument();
  });

  it('persists XML + preview when the editor posts an export message', async () => {
    renderAt('/diagrams/d1');
    await screen.findByTitle('draw.io editor');
    window.dispatchEvent(
      new MessageEvent('message', {
        origin: 'https://embed.diagrams.net',
        data: JSON.stringify({ event: 'save', xml: '<mxGraphModel>new</mxGraphModel>' }),
      }),
    );
    window.dispatchEvent(
      new MessageEvent('message', {
        origin: 'https://embed.diagrams.net',
        data: JSON.stringify({ event: 'export', data: 'data:image/svg+xml;base64,PHN2Zz4=' }),
      }),
    );
    await waitFor(() =>
      expect(apiFetch).toHaveBeenCalledWith(
        '/diagrams/d1',
        expect.objectContaining({ method: 'PATCH', body: expect.stringContaining('preview') }),
      ),
    );
  });
});
