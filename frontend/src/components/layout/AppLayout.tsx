import { Suspense, useEffect, useRef, useState, type CSSProperties } from 'react';
import { Outlet, useLocation } from 'react-router-dom';

import { ErrorBoundary } from '@/components/ErrorBoundary';
import { Spinner } from '@/components/ui/spinner';
import { AgentDock } from '@/features/cli/AgentDock';
import { cn } from '@/lib/cn';
import { useAgentDock } from '@/stores/agentDock';
import { isVisualEditorDetailRoute } from '@/features/objects/editorLayout';
import { Rail } from './Rail';
import { Sidebar } from './Sidebar';
import { Header } from './Header';

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * App shell: a fixed sidebar on desktop, an overlay drawer on mobile, and a
 * sticky header above the routed content. The mobile drawer is a proper modal —
 * focus is moved in, trapped, restored on close, and background scroll is locked.
 */
export function AppLayout() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const drawerRef = useRef<HTMLElement | null>(null);
  const triggerRef = useRef<HTMLElement | null>(null);

  // The docked agent pane shows on every feature page (the standalone `/agent`
  // route already *is* the panel, and the butler home `/` is a full-page agent
  // surface itself, so don't double it up on either).
  const dockOpen = useAgentDock((s) => s.open);
  const dockWidth = useAgentDock((s) => s.width);
  const { pathname } = useLocation();
  const showDock = dockOpen && pathname !== '/agent' && pathname !== '/';
  const immersive = isVisualEditorDetailRoute(pathname);

  const openDrawer = () => {
    triggerRef.current = document.activeElement as HTMLElement | null;
    setMobileOpen(true);
  };
  const closeDrawer = () => setMobileOpen(false);

  useEffect(() => {
    if (!mobileOpen) return;

    const drawer = drawerRef.current;
    const focusables = drawer ? Array.from(drawer.querySelectorAll<HTMLElement>(FOCUSABLE)) : [];
    focusables[0]?.focus();

    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        closeDrawer();
        return;
      }
      if (e.key === 'Tab' && focusables.length > 0) {
        const first = focusables[0];
        const last = focusables[focusables.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };
    document.addEventListener('keydown', onKey);

    return () => {
      document.removeEventListener('keydown', onKey);
      document.body.style.overflow = prevOverflow;
      triggerRef.current?.focus();
    };
  }, [mobileOpen]);

  return (
    <div
      className="min-h-svh bg-background"
      style={{ '--agent-dock-w': `${showDock ? dockWidth : 0}px` } as CSSProperties}
    >
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:left-2 focus:top-2 focus:z-50 focus:rounded-md focus:bg-card focus:px-3 focus:py-2 focus:shadow"
      >
        Skip to content
      </a>

      {/* Desktop icon rail */}
      <aside className="fixed inset-y-0 left-0 z-30 hidden w-16 border-r border-border bg-card md:block">
        <Rail />
      </aside>

      {/* Mobile drawer */}
      {mobileOpen && (
        <div
          className="fixed inset-0 z-40 md:hidden"
          role="dialog"
          aria-modal="true"
          aria-label="Navigation"
        >
          <button
            type="button"
            className="absolute inset-0 bg-black/40"
            aria-label="Close navigation"
            onClick={closeDrawer}
          />
          <aside
            ref={drawerRef}
            className="absolute inset-y-0 left-0 w-72 border-r border-border bg-card"
          >
            <Sidebar onNavigate={closeDrawer} />
          </aside>
        </div>
      )}

      <div
        className={cn(
          'md:pl-16',
          showDock && 'lg:[padding-right:var(--agent-dock-w)]',
          immersive && 'flex h-svh flex-col',
        )}
      >
        <Header onMenuClick={openDrawer} />
        <main
          id="main"
          tabIndex={-1}
          className={cn(
            'w-full',
            immersive
              ? 'flex min-h-0 flex-1 flex-col px-2 py-2 sm:px-3'
              : 'mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-8',
          )}
        >
          <ErrorBoundary>
            <Suspense
              fallback={
                <div className="grid place-items-center py-16">
                  <Spinner label="Loading" />
                </div>
              }
            >
              <div className={immersive ? 'flex min-h-0 flex-1 flex-col' : undefined}>
                <Outlet />
              </div>
            </Suspense>
          </ErrorBoundary>
        </main>
      </div>

      {showDock && <AgentDock />}
    </div>
  );
}
