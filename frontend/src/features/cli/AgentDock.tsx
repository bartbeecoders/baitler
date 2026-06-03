import { useState, type KeyboardEvent, type PointerEvent } from 'react';
import { useLocation } from 'react-router-dom';
import { Bot, X } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/cn';
import { DOCK_MAX_WIDTH, DOCK_MIN_WIDTH, useAgentDock } from '@/stores/agentDock';
import { AgentPanel } from './AgentPanel';
import { pageContext } from './context';

/** The right-hand agent pane, rendered on feature pages. A fixed slide-over on
 * small screens (with a backdrop); a side column on large screens (the layout
 * reserves space via `lg:pr-96`). */
export function AgentDock() {
  const setOpen = useAgentDock((s) => s.setOpen);
  const width = useAgentDock((s) => s.width);
  const setWidth = useAgentDock((s) => s.setWidth);
  const [dragging, setDragging] = useState(false);
  const { pathname } = useLocation();
  const { label, context } = pageContext(pathname);

  // Drag the left edge to resize. The pane is right-anchored, so the width is the
  // distance from the pointer to the right viewport edge (capped to the viewport).
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    setDragging(true);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging) return;
    const max = Math.min(DOCK_MAX_WIDTH, window.innerWidth - 40);
    setWidth(Math.min(window.innerWidth - e.clientX, max));
  };
  const endDrag = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging) return;
    setDragging(false);
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* capture may already be released */
    }
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // Arrow-left widens (handle moves left), arrow-right narrows.
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      setWidth(width + 16);
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      setWidth(width - 16);
    }
  };

  return (
    <>
      {/* Backdrop (small screens only — on lg the layout makes room beside it). */}
      <button
        type="button"
        aria-label="Close agent"
        className="fixed inset-0 z-30 bg-black/40 lg:hidden"
        onClick={() => setOpen(false)}
      />
      <aside
        aria-label="Agent"
        style={{ width }}
        className="fixed right-0 top-0 z-40 flex h-svh max-w-[100vw] flex-col border-l border-border bg-card"
      >
        {/* Resize handle on the left edge. */}
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize agent pane"
          aria-valuenow={Math.round(width)}
          aria-valuemin={DOCK_MIN_WIDTH}
          aria-valuemax={DOCK_MAX_WIDTH}
          tabIndex={0}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onKeyDown={onKeyDown}
          className={cn(
            'absolute left-0 top-0 z-10 h-full w-2 -translate-x-1 cursor-col-resize touch-none hover:bg-primary/40 focus-visible:bg-primary/40 focus:outline-none',
            dragging && 'bg-primary/40',
          )}
        />
        <div className="flex h-16 shrink-0 items-center justify-between gap-2 border-b border-border px-4">
          <div className="flex items-center gap-2">
            <Bot className="h-4 w-4 text-primary" aria-hidden="true" />
            <span className="text-sm font-semibold">Agent</span>
            <Badge variant="muted">{label}</Badge>
          </div>
          <Button variant="ghost" size="icon" aria-label="Close agent" onClick={() => setOpen(false)}>
            <X className="h-4 w-4" />
          </Button>
        </div>
        <div className="min-h-0 flex-1 overflow-hidden p-3">
          <AgentPanel embedded contextLabel={label} context={context} />
        </div>
      </aside>
    </>
  );
}
