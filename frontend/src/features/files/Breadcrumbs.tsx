import { ChevronRight, Home } from 'lucide-react';

import { cn } from '@/lib/cn';
import type { FolderItem } from './types';

interface Props {
  breadcrumbs: FolderItem[];
  onNavigate: (folderId: string | null) => void;
}

/** Root → … → current folder navigation. */
export function Breadcrumbs({ breadcrumbs, onNavigate }: Props) {
  return (
    <nav aria-label="Breadcrumb" className="flex items-center gap-1 text-sm">
      <button
        type="button"
        onClick={() => onNavigate(null)}
        className={cn(
          'flex items-center gap-1 rounded px-1.5 py-1 hover:bg-muted',
          breadcrumbs.length === 0 ? 'font-medium text-foreground' : 'text-muted-foreground',
        )}
      >
        <Home className="h-4 w-4" aria-hidden="true" />
        Home
      </button>
      {breadcrumbs.map((crumb, i) => {
        const isLast = i === breadcrumbs.length - 1;
        return (
          <span key={crumb.id} className="flex items-center gap-1">
            <ChevronRight className="h-4 w-4 text-muted-foreground" aria-hidden="true" />
            <button
              type="button"
              onClick={() => onNavigate(crumb.id)}
              aria-current={isLast ? 'page' : undefined}
              className={cn(
                'rounded px-1.5 py-1 hover:bg-muted',
                isLast ? 'font-medium text-foreground' : 'text-muted-foreground',
              )}
            >
              {crumb.name}
            </button>
          </span>
        );
      })}
    </nav>
  );
}
