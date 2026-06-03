import { useState } from 'react';
import { Plus, RefreshCw, Search, Trash2 } from 'lucide-react';

import { Badge, type BadgeProps } from '@/components/ui/badge';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';

/** One selectable row in an object list. */
export interface ObjectListItem {
  id: string;
  title: string;
  badge?: { label: string; variant?: BadgeProps['variant'] };
}

/** A dropdown filter rendered in the list's filter bar. */
export interface FilterConfig {
  key: string;
  /** Accessible label (also the "all" option text). */
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
}

export interface ObjectListProps {
  items: ObjectListItem[];
  isLoading: boolean;
  isError?: boolean;
  /** The currently open item (highlighted; target of the trash button). */
  activeId: string | null;
  search: string;
  onSearch: (value: string) => void;
  filters?: FilterConfig[];
  onSelect: (id: string) => void;
  onCreate: () => void;
  creating?: boolean;
  /** Delete a specific item (the trash button deletes the active one). */
  onDelete: (id: string) => void;
  deleting?: boolean;
  onRefresh: () => void;
  refreshing?: boolean;
  /** Singular noun for empty/aria copy, e.g. "document". */
  noun: string;
}

const ICON_BTN =
  'inline-flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-40 disabled:hover:bg-transparent';
const SELECT_CLASS =
  'h-8 w-full rounded-md border border-border bg-transparent px-2 text-xs focus:outline-none focus:ring-2 focus:ring-ring';

/**
 * The reusable object list shown inside an expanded "Objects" sidebar section:
 * a control row (new / refresh / delete-active), a search box, optional filter
 * dropdowns, and a scrollable item list. Purely presentational — all data and
 * actions are supplied by the per-type adapter.
 */
export function ObjectList({
  items,
  isLoading,
  isError,
  activeId,
  search,
  onSearch,
  filters = [],
  onSelect,
  onCreate,
  creating,
  onDelete,
  deleting,
  onRefresh,
  refreshing,
  noun,
}: ObjectListProps) {
  // Local confirm gate for the destructive trash button.
  const [confirming, setConfirming] = useState(false);

  return (
    <div className="flex flex-col gap-1.5 pb-1">
      <div className="flex items-center gap-1">
        <button type="button" className={ICON_BTN} onClick={onCreate} disabled={creating} aria-label={`New ${noun}`}>
          <Plus className="h-4 w-4" aria-hidden="true" />
        </button>
        <button
          type="button"
          className={ICON_BTN}
          onClick={onRefresh}
          aria-label={`Refresh ${noun} list`}
        >
          <RefreshCw className={cn('h-4 w-4', refreshing && 'animate-spin')} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={cn(ICON_BTN, confirming && 'text-danger')}
          onClick={() => {
            if (!activeId) return;
            if (confirming) {
              onDelete(activeId);
              setConfirming(false);
            } else {
              setConfirming(true);
            }
          }}
          disabled={!activeId || deleting}
          aria-label={confirming ? `Confirm delete ${noun}` : `Delete selected ${noun}`}
          title={confirming ? 'Click again to confirm' : 'Delete the selected item'}
        >
          <Trash2 className="h-4 w-4" aria-hidden="true" />
        </button>
        {confirming && (
          <button
            type="button"
            className="text-xs text-muted-foreground hover:text-foreground"
            onClick={() => setConfirming(false)}
          >
            cancel
          </button>
        )}
      </div>

      <div className="relative">
        <Search
          className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground"
          aria-hidden="true"
        />
        <input
          type="search"
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          placeholder={`Search ${noun}s…`}
          aria-label={`Search ${noun}s`}
          className="h-8 w-full rounded-md border border-border bg-transparent pl-7 pr-2 text-xs placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      {filters.length > 0 && (
        <div className="flex flex-col gap-1">
          {filters.map((f) => (
            <select
              key={f.key}
              className={SELECT_CLASS}
              value={f.value}
              onChange={(e) => f.onChange(e.target.value)}
              aria-label={f.label}
            >
              <option value="">{f.label}</option>
              {f.options.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          ))}
        </div>
      )}

      <div className="max-h-64 overflow-auto rounded-md border border-border">
        {isLoading ? (
          <div className="grid place-items-center py-6">
            <Spinner label={`Loading ${noun}s`} />
          </div>
        ) : isError ? (
          <p className="px-2 py-3 text-xs text-danger" role="alert">
            Failed to load {noun}s.
          </p>
        ) : items.length === 0 ? (
          <p className="px-2 py-3 text-xs text-muted-foreground">No {noun}s match.</p>
        ) : (
          <ul>
            {items.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => onSelect(item.id)}
                  className={cn(
                    'flex w-full items-center justify-between gap-2 px-2 py-1.5 text-left text-xs',
                    item.id === activeId
                      ? 'bg-accent text-accent-foreground'
                      : 'hover:bg-muted',
                  )}
                  aria-current={item.id === activeId}
                >
                  <span className="truncate">{item.title}</span>
                  {item.badge && (
                    <Badge variant={item.badge.variant} className="shrink-0 px-1.5 py-0">
                      {item.badge.label}
                    </Badge>
                  )}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
