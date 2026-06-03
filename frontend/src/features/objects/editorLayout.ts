/** Routes that use a full-pane visual canvas when a detail id is open. */
export const VISUAL_EDITOR_BASES = ['/diagrams', '/mindmaps'] as const;

/** True for `/diagrams/:id` and `/mindmaps/:id` (not the empty list hint). */
export function isVisualEditorDetailRoute(pathname: string): boolean {
  for (const base of VISUAL_EDITOR_BASES) {
    if (pathname.startsWith(`${base}/`) && pathname.length > base.length + 1) return true;
  }
  return false;
}

/** Fills the immersive main column below the title/tags chrome. */
export const OBJECT_EDITOR_ROOT = 'flex min-h-0 flex-1 flex-col gap-2';

/** Canvas host: grows to fill remaining editor height. */
export const OBJECT_CANVAS =
  'relative min-h-0 flex-1 overflow-hidden rounded-lg border border-border';