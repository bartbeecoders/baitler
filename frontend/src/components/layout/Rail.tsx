import { useEffect, useState } from 'react';
import { Link, NavLink, useLocation } from 'react-router-dom';
import { Boxes } from 'lucide-react';

import { navItems } from '@/config/navigation';
import { OBJECT_BASE_PATHS } from '@/config/objects';
import { ObjectsNav } from '@/features/objects/ObjectsNav';
import { PluginRailItems } from '@/features/plugins/slots';
import { cn } from '@/lib/cn';
import { Brand } from './Brand';

/**
 * Desktop navigation as a thin icon rail (the butler home is the main surface,
 * so chrome stays out of the way). Content types live behind the "Objects"
 * button, which opens a flyout panel with the per-type lists; everything else is
 * a direct icon link. Mobile keeps the full drawer (see `Sidebar`).
 */
export function Rail() {
  const { pathname } = useLocation();
  const [objectsOpen, setObjectsOpen] = useState(false);

  // Navigating (from the flyout or anywhere) closes the flyout — adjust state
  // during render rather than via an effect.
  const [lastPathname, setLastPathname] = useState(pathname);
  if (lastPathname !== pathname) {
    setLastPathname(pathname);
    setObjectsOpen(false);
  }

  useEffect(() => {
    if (!objectsOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setObjectsOpen(false);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [objectsOpen]);

  const primaryItems = navItems.filter((item) => !OBJECT_BASE_PATHS.includes(item.path));
  const onObjectRoute = OBJECT_BASE_PATHS.some(
    (base) => pathname === base || pathname.startsWith(`${base}/`),
  );

  return (
    <>
      <nav aria-label="Primary" className="flex h-full flex-col items-center gap-1 py-3">
        <Link to="/" className="mb-2 rounded-md p-1" aria-label="Baitler home">
          <Brand showName={false} />
        </Link>

        {primaryItems.map((item) => (
          <span key={item.path} className="contents">
            <NavLink
              to={item.path}
              end={item.path === '/'}
              title={item.label}
              aria-label={item.label}
              className={({ isActive }) =>
                cn(
                  'grid h-11 w-11 place-items-center rounded-lg transition-colors',
                  isActive
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:bg-muted hover:text-foreground',
                )
              }
            >
              <item.icon className="h-5 w-5" aria-hidden="true" />
            </NavLink>
            {/* The Objects flyout trigger sits right under Files, like the old group. */}
            {item.path === '/files' && (
              <button
                type="button"
                onClick={() => setObjectsOpen((v) => !v)}
                title="Objects"
                aria-label="Objects"
                aria-expanded={objectsOpen}
                className={cn(
                  'grid h-11 w-11 place-items-center rounded-lg transition-colors',
                  objectsOpen || onObjectRoute
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:bg-muted hover:text-foreground',
                )}
              >
                <Boxes className="h-5 w-5" aria-hidden="true" />
              </button>
            )}
          </span>
        ))}
        {/* Enabled plugins with a `rail` UI mount add their own icons (16.C). */}
        <PluginRailItems />
      </nav>

      {/* Objects flyout */}
      {objectsOpen && (
        <>
          <button
            type="button"
            className="fixed inset-0 z-30 cursor-default"
            aria-label="Close objects panel"
            onClick={() => setObjectsOpen(false)}
            tabIndex={-1}
          />
          <div
            role="dialog"
            aria-label="Objects"
            className="fixed inset-y-0 left-16 z-40 w-80 overflow-y-auto border-r border-border bg-card p-3 shadow-lg"
          >
            <ul>
              <ObjectsNav />
            </ul>
          </div>
        </>
      )}
    </>
  );
}
