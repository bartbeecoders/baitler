import { describe, expect, it } from 'vitest';

import { detectRootedPath } from './workspacePaths';

const ROOTS = ['/run/media/bart/Development', '/home/bart/Documents/'];

describe('detectRootedPath', () => {
  it('finds a typed path under an allow-listed root', () => {
    expect(
      detectRootedPath('I mean /run/media/bart/Development/Projects', ROOTS),
    ).toBe('/run/media/bart/Development/Projects');
  });

  it('matches a root itself and normalizes trailing slashes', () => {
    expect(detectRootedPath('list /home/bart/Documents/ please', ROOTS)).toBe(
      '/home/bart/Documents',
    );
  });

  it('strips trailing sentence punctuation', () => {
    expect(
      detectRootedPath('what is in /run/media/bart/Development/Projects?', ROOTS),
    ).toBe('/run/media/bart/Development/Projects');
  });

  it('ignores paths outside the roots', () => {
    expect(detectRootedPath('look at /etc/passwd and /tmp/x', ROOTS)).toBeNull();
    // Prefix of a root is not under it.
    expect(detectRootedPath('/run/media/bart/Devel', ROOTS)).toBeNull();
    // Sibling sharing the root as a string prefix is not under it.
    expect(detectRootedPath('/run/media/bart/Development2/x', ROOTS)).toBeNull();
  });

  it('returns null with no roots or no path in the text', () => {
    expect(detectRootedPath('/run/media/bart/Development/x', [])).toBeNull();
    expect(detectRootedPath('organise my ideas', ROOTS)).toBeNull();
  });
});
