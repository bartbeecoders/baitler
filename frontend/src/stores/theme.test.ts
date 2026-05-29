import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';

import { useApplyTheme, useThemeStore } from './theme';

describe('theme store', () => {
  beforeEach(() => {
    useThemeStore.setState({ theme: 'light' });
  });

  it('toggles between light and dark', () => {
    useThemeStore.getState().toggle();
    expect(useThemeStore.getState().theme).toBe('dark');
    useThemeStore.getState().toggle();
    expect(useThemeStore.getState().theme).toBe('light');
  });

  it('sets the theme explicitly', () => {
    useThemeStore.getState().setTheme('dark');
    expect(useThemeStore.getState().theme).toBe('dark');
  });
});

describe('useApplyTheme', () => {
  beforeEach(() => {
    useThemeStore.setState({ theme: 'light' });
    document.documentElement.classList.remove('dark');
  });

  it('reflects the current theme onto <html>', () => {
    useThemeStore.setState({ theme: 'dark' });
    renderHook(() => useApplyTheme());
    expect(document.documentElement.classList.contains('dark')).toBe(true);

    act(() => useThemeStore.getState().setTheme('light'));
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('light');
  });
});
