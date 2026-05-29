import { NavLink } from 'react-router-dom';

import { navItems } from '@/config/navigation';
import { cn } from '@/lib/cn';
import { Brand } from './Brand';

/** Primary navigation sidebar. `onNavigate` lets the mobile drawer close on tap. */
export function Sidebar({ onNavigate }: { onNavigate?: () => void }) {
  return (
    <nav aria-label="Primary" className="flex h-full flex-col p-4">
      <div className="px-2 py-3">
        <Brand />
      </div>

      <ul className="mt-4 flex flex-col gap-1">
        {navItems.map((item) => (
          <li key={item.path}>
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
        ))}
      </ul>

      <p className="mt-auto px-3 py-2 text-xs text-muted-foreground">Baitler · v0.1</p>
    </nav>
  );
}
