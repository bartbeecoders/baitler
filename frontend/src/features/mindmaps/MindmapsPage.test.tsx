import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { MindmapsPage } from './MindmapsPage';

const MINDMAP = {
  id: 'm1',
  title: 'Roadmap',
  graph: { nodes: [{ id: 'n1', label: 'Root' }], edges: [] },
  source_format: 'json',
  folder_id: null,
  project_id: null,
  tags: [],
  review: 'published',
  version: 1,
  published_at: null,
  created_at: '2026-06-01T00:00:00Z',
  updated_at: '2026-06-01T00:00:00Z',
};

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/mindmaps/m1') return MINDMAP;
    return {};
  }),
}));

// Stub the heavy React Flow canvas; the page wraps it in `mindmap-canvas`.
vi.mock('@xyflow/react', () => ({
  ReactFlow: () => null,
  Background: () => null,
  Controls: () => null,
  MiniMap: () => null,
  addEdge: (c: unknown, eds: unknown[]) => [...eds, c],
  useNodesState: (initial: unknown[]) => [initial, () => {}, () => {}],
  useEdgesState: (initial: unknown[]) => [initial, () => {}, () => {}],
}));

function renderAt(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/mindmaps" element={<MindmapsPage />} />
          <Route path="/mindmaps/:id" element={<MindmapsPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('MindmapsPage (editor route)', () => {
  it('shows the empty hint when no mindmap is selected', () => {
    renderAt('/mindmaps');
    expect(screen.getByText(/select a mindmap/i)).toBeInTheDocument();
  });

  it('renders the canvas editor for the routed mindmap', async () => {
    renderAt('/mindmaps/m1');
    await waitFor(() => expect(screen.getByLabelText('Mindmap title')).toHaveValue('Roadmap'));
    expect(screen.getByTestId('mindmap-canvas')).toBeInTheDocument();
  });
});
