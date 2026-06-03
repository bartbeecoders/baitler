import { describe, expect, it } from 'vitest';

import { isVisualEditorDetailRoute } from './editorLayout';

describe('isVisualEditorDetailRoute', () => {
  it('is true for diagram and mindmap detail routes', () => {
    expect(isVisualEditorDetailRoute('/diagrams/abc')).toBe(true);
    expect(isVisualEditorDetailRoute('/mindmaps/abc')).toBe(true);
  });

  it('is false for list routes and other pages', () => {
    expect(isVisualEditorDetailRoute('/diagrams')).toBe(false);
    expect(isVisualEditorDetailRoute('/mindmaps')).toBe(false);
    expect(isVisualEditorDetailRoute('/editor/doc-1')).toBe(false);
  });
});