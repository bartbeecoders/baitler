import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { FilesPage } from './FilesPage';
import type { Listing } from './types';

const listing: Listing = {
  folder: null,
  breadcrumbs: [],
  folders: [
    {
      id: 'f1',
      name: 'Reports',
      parent_id: null,
      created_at: '2026-05-29T00:00:00Z',
      updated_at: '2026-05-29T00:00:00Z',
    },
  ],
  files: [
    {
      id: 'a1',
      name: 'budget.txt',
      mime: 'text/plain',
      size: 2048,
      folder_id: null,
      created_at: '2026-05-29T00:00:00Z',
      updated_at: '2026-05-29T00:00:00Z',
    },
  ],
};

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async () => listing),
  apiFetchBlob: vi.fn(async () => new Blob()),
}));

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <FilesPage />
    </QueryClientProvider>,
  );
}

describe('FilesPage', () => {
  it('renders folders and files from the listing', async () => {
    renderPage();
    expect(screen.getByRole('heading', { name: 'Files' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('Reports')).toBeInTheDocument());
    expect(screen.getByText('budget.txt')).toBeInTheDocument();
    // Human-readable size is shown for files.
    expect(screen.getByText('2.0 KB')).toBeInTheDocument();
  });

  it('exposes upload and new-folder controls', () => {
    renderPage();
    expect(screen.getByRole('button', { name: /upload/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /new folder/i })).toBeInTheDocument();
  });
});
