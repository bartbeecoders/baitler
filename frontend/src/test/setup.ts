// Extends Vitest's `expect` with jest-dom matchers (toBeInTheDocument, etc.)
// and registers their TypeScript augmentation.
import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

// With Vitest `globals: false`, Testing Library's automatic cleanup is not
// registered — do it explicitly so rendered DOM doesn't leak between tests.
afterEach(() => {
  cleanup();
});

// jsdom does not implement matchMedia; stub it so theme resolution (systemTheme /
// useApplyTheme) works in tests. Defaults to light; override per-test if needed.
if (!window.matchMedia) {
  vi.stubGlobal(
    'matchMedia',
    (query: string): MediaQueryList =>
      ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: () => {},
        removeEventListener: () => {},
        addListener: () => {},
        removeListener: () => {},
        dispatchEvent: () => false,
      }) as unknown as MediaQueryList,
  );
}
