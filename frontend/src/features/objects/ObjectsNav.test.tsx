import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { ObjectsNav } from './ObjectsNav';
import { useObjectsNav } from '@/stores/objectsNav';

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/documents') return { documents: [{ id: 'd1', title: 'Memo', tags: [] }] };
    return {};
  }),
}));

function renderNav() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/']}>
        <ul>
          <ObjectsNav />
        </ul>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('ObjectsNav', () => {
  beforeEach(() => {
    // Reset persisted accordion state to a known baseline.
    useObjectsNav.setState({ groupOpen: true, openType: 'documents' });
  });

  it('renders the Objects group with each content type', () => {
    renderNav();
    expect(screen.getByRole('button', { name: 'Objects' })).toBeInTheDocument();
    for (const label of ['Documents', 'Ideas', 'Pages', 'Mindmaps', 'Diagrams']) {
      expect(screen.getByRole('button', { name: label })).toBeInTheDocument();
    }
  });

  it('mounts the list adapter for the open type (Documents) with its controls + items', async () => {
    renderNav();
    expect(screen.getByLabelText('New document')).toBeInTheDocument();
    expect(screen.getByLabelText('Search documents')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('Memo')).toBeInTheDocument());
  });

  it('collapses the group when its header is toggled', () => {
    renderNav();
    fireEvent.click(screen.getByRole('button', { name: 'Objects' }));
    expect(screen.queryByRole('button', { name: 'Documents' })).not.toBeInTheDocument();
  });
});
