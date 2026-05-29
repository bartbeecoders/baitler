import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { AiPage } from './AiPage';

const providers = [
  {
    id: 'mock',
    label: 'Mock',
    requires_key: false,
    configured: true,
    models: [{ id: 'mock-1', label: 'Mock model', modalities: ['text'] }],
  },
  {
    id: 'openai',
    label: 'OpenAI',
    requires_key: true,
    configured: false,
    models: [{ id: 'gpt-4o', label: 'GPT-4o', modalities: ['text'] }],
  },
];

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/ai/providers') return { providers };
    throw new Error(`unexpected path: ${path}`);
  }),
  apiFetchBlob: vi.fn(),
}));

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AiPage />
    </QueryClientProvider>,
  );
}

describe('AiPage', () => {
  it('renders the chat shell and provider options', async () => {
    renderPage();
    expect(screen.getByRole('heading', { name: 'AI' })).toBeInTheDocument();
    expect(screen.getByText(/start a conversation/i)).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole('option', { name: 'Mock' })).toBeInTheDocument(),
    );
    expect(screen.getByRole('option', { name: 'OpenAI' })).toBeInTheDocument();
  });

  it('prompts for an API key when an unconfigured provider is selected', async () => {
    renderPage();
    await waitFor(() => screen.getByRole('option', { name: 'OpenAI' }));
    await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Provider' }), 'openai');
    expect(screen.getByText(/needs an API key/i)).toBeInTheDocument();
  });
});
