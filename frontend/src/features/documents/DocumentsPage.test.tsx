import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { DocumentsPage } from './DocumentsPage';

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  // Empty list keeps the (heavy) rich-text editor from auto-mounting in jsdom.
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/documents') return { documents: [] };
    throw new Error(`unexpected path: ${path}`);
  }),
  apiFetchBlob: vi.fn(),
}));

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <DocumentsPage />
    </QueryClientProvider>,
  );
}

describe('DocumentsPage', () => {
  it('shows the New control and empty states', async () => {
    renderPage();
    expect(screen.getByRole('button', { name: /new/i })).toBeInTheDocument();
    expect(screen.getByText(/select a document/i)).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText(/no documents yet/i)).toBeInTheDocument());
  });
});
