import { useEffect, useState } from 'react';
import { LayoutGrid, List, Plus, Search } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import { errorMessage, useIdeas, useTags } from './api';
import { IdeaCard } from './IdeaCard';
import { IdeaEditorModal } from './IdeaEditorModal';
import { STATUSES, STATUS_LABELS, type IdeaStatus } from './types';

function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return debounced;
}

const SELECT_CLASS = 'h-10 rounded-md border border-input bg-background px-2 text-sm';

export function IdeasPage() {
  const [view, setView] = useState<'list' | 'board'>('list');
  const [statusFilter, setStatusFilter] = useState<IdeaStatus | ''>('');
  const [tagFilter, setTagFilter] = useState('');
  const [q, setQ] = useState('');
  const debouncedQ = useDebounced(q.trim(), 250);
  const [editor, setEditor] = useState<{ open: boolean; id: string | null }>({
    open: false,
    id: null,
  });

  // Board view groups by status, so it ignores the status filter.
  const filters = {
    status: view === 'list' && statusFilter ? statusFilter : null,
    tag: tagFilter || null,
    q: debouncedQ,
  };
  const ideasQuery = useIdeas(filters);
  const { data: tags = [] } = useTags();
  const ideas = ideasQuery.data ?? [];

  const openNew = () => setEditor({ open: true, id: null });
  const openIdea = (id: string) => setEditor({ open: true, id });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Ideas</h1>
          <p className="text-sm text-muted-foreground">Capture, tag, and link your ideas.</p>
        </div>
        <Button onClick={openNew}>
          <Plus className="h-4 w-4" aria-hidden="true" />
          New idea
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <div className="relative">
          <Search
            className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            type="search"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search ideas…"
            aria-label="Search ideas"
            className="w-48 pl-8"
          />
        </div>

        {view === 'list' && (
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value as IdeaStatus | '')}
            aria-label="Filter by status"
            className={SELECT_CLASS}
          >
            <option value="">All statuses</option>
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {STATUS_LABELS[s]}
              </option>
            ))}
          </select>
        )}

        <select
          value={tagFilter}
          onChange={(e) => setTagFilter(e.target.value)}
          aria-label="Filter by tag"
          className={SELECT_CLASS}
        >
          <option value="">All tags</option>
          {tags.map((tag) => (
            <option key={tag} value={tag}>
              {tag}
            </option>
          ))}
        </select>

        <div className="ml-auto flex rounded-md border border-border p-0.5">
          <ViewToggle active={view === 'list'} onClick={() => setView('list')} label="List view">
            <List className="h-4 w-4" />
          </ViewToggle>
          <ViewToggle active={view === 'board'} onClick={() => setView('board')} label="Board view">
            <LayoutGrid className="h-4 w-4" />
          </ViewToggle>
        </div>
      </div>

      {ideasQuery.isLoading ? (
        <div className="grid place-items-center py-16 text-muted-foreground">
          <Spinner label="Loading ideas" />
        </div>
      ) : ideasQuery.isError ? (
        <p className="py-16 text-center text-sm text-danger" role="alert">
          {errorMessage(ideasQuery.error)}
        </p>
      ) : ideas.length === 0 ? (
        <div className="grid place-items-center gap-1 py-16 text-center">
          <p className="font-medium">No ideas yet</p>
          <p className="text-sm text-muted-foreground">Create one to get started.</p>
        </div>
      ) : view === 'list' ? (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {ideas.map((idea) => (
            <IdeaCard key={idea.id} idea={idea} onOpen={() => openIdea(idea.id)} />
          ))}
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
          {STATUSES.map((s) => {
            const column = ideas.filter((i) => i.status === s);
            return (
              <section key={s} aria-label={STATUS_LABELS[s]} className="flex flex-col gap-2">
                <h2 className="flex items-center justify-between px-1 text-sm font-semibold">
                  {STATUS_LABELS[s]}
                  <span className="text-xs font-normal text-muted-foreground">{column.length}</span>
                </h2>
                <div className="flex flex-col gap-2 rounded-lg bg-muted/40 p-2">
                  {column.map((idea) => (
                    <IdeaCard
                      key={idea.id}
                      idea={idea}
                      onOpen={() => openIdea(idea.id)}
                      showStatus={false}
                    />
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      )}

      <IdeaEditorModal
        ideaId={editor.id}
        open={editor.open}
        onClose={() => setEditor({ open: false, id: null })}
      />
    </div>
  );
}

function ViewToggle({
  active,
  onClick,
  label,
  children,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      aria-pressed={active}
      className={cn(
        'rounded p-1.5',
        active ? 'bg-muted text-foreground' : 'text-muted-foreground hover:text-foreground',
      )}
    >
      {children}
    </button>
  );
}
