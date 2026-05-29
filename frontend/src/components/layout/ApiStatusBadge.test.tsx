import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { UseQueryResult } from '@tanstack/react-query';

import { ApiStatusBadge } from './ApiStatusBadge';
import { useHealth } from '@/hooks/useSystemStatus';
import type { HealthResponse } from '@/types/api';

// Mock only the query hook; keep the real `apiStatusOf` derivation.
vi.mock('@/hooks/useSystemStatus', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/hooks/useSystemStatus')>();
  return { ...actual, useHealth: vi.fn() };
});

const mockUseHealth = vi.mocked(useHealth);

function asQuery(partial: Partial<UseQueryResult<HealthResponse>>) {
  return partial as unknown as UseQueryResult<HealthResponse>;
}

describe('ApiStatusBadge', () => {
  it('shows a checking state while loading', () => {
    mockUseHealth.mockReturnValue(asQuery({ isLoading: true, isError: false, data: undefined }));
    render(<ApiStatusBadge />);
    expect(screen.getByText(/checking api/i)).toBeInTheDocument();
  });

  it('shows offline when the query errors', () => {
    mockUseHealth.mockReturnValue(asQuery({ isLoading: false, isError: true, data: undefined }));
    render(<ApiStatusBadge />);
    expect(screen.getByText(/api offline/i)).toBeInTheDocument();
  });

  it('shows connected when the API reports ok', () => {
    mockUseHealth.mockReturnValue(
      asQuery({ isLoading: false, isError: false, data: { status: 'ok', db: 'up' } }),
    );
    render(<ApiStatusBadge />);
    expect(screen.getByText(/api connected/i)).toBeInTheDocument();
  });
});
