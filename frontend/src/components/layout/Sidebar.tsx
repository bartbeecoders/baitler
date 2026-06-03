import { Fragment } from 'react';
import { NavLink } from 'react-router-dom';

import { navItems } from '@/config/navigation';
import { OBJECT_BASE_PATHS } from '@/config/objects';
import { ObjectsNav } from '@/features/objects/ObjectsNav';
import { cn } from '@/lib/cn';
import { Brand } from './Brand';

/** Primary navigation sidebar. `onNavigate` lets the mobile drawer close on tap. */
export function Sidebar({ onNavigate }: { onNavigate?: () => void }) {
  // Object types (Documents/Ideas/Pages/Mindmaps/Diagrams) live in the "Objects"
  // group, not the flat primary nav.
  const primaryItems = navItems.filter((item) => !OBJECT_BASE_PATHS.includes(item.path));

  return (
    <nav aria-label="Primary" className="flex h-full flex-col p-4">
      <div className="px-2 py-3">
        <Brand />
      </div>

      <ul className="mt-4 flex flex-1 flex-col gap-1 overflow-y-auto">
        {primaryItems.map((item) => (
          <Fragment key={item.path}>
            <li>
              <NavLink
                to={item.path}
                end={item.path === '/'}
                onClick={onNavigate}
                className={({ isActive }) =>
                  cn(
                    'flex items-center gap-3 rounded-md border-l-2 px-3 py-2 text-sm font-medium transition-colors',
                    isActive
                      ? 'border-primary bg-accent text-accent-foreground'
                      : 'border-transparent text-muted-foreground hover:bg-muted hover:text-foreground',
                  )
                }
              >
                <item.icon className="h-5 w-5 shrink-0" aria-hidden="true" />
                {item.label}
              </NavLink>
            </li>
            {/* The "Objects" group sits right under Files. */}
            {item.path === '/files' && <ObjectsNav />}
          </Fragment>
        ))}
      </ul>

      <p className="mt-2 px-3 py-2 text-xs text-muted-foreground">Baitler · v0.1</p>
    </nav>
  );
}
