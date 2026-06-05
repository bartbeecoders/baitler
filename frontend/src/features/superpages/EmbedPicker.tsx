import { useEffect, useState } from 'react';

import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import { useDiagrams } from '@/features/diagrams/api';
import { useDocuments } from '@/features/documents/api';
import { useListing } from '@/features/files/api';
import { useIdeas } from '@/features/ideas/api';
import { useMindmaps } from '@/features/mindmaps/api';
import { usePages } from '@/features/pages/api';
import { EMBED_TYPE_LABELS, type EmbedItemType } from './types';

function useDebounced<T>(value: T, ms = 200): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

interface PickerProps {
  open: boolean;
  itemType: EmbedItemType;
  onClose: () => void;
  onPick: (id: string, title: string) => void;
}

/** Searchable item chooser — replaces pasting raw item ids into embeds. */
export function EmbedPicker({ open, itemType, onClose, onPick }: PickerProps) {
  const [search, setSearch] = useState('');
  const q = useDebounced(search.trim());

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={`Embed a ${EMBED_TYPE_LABELS[itemType]}`}
      className="max-w-lg"
    >
      <Input
        autoFocus
        placeholder={`Search ${EMBED_TYPE_LABELS[itemType].toLowerCase()}s…`}
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        aria-label="Search items"
      />
      <div className="mt-3 max-h-80 divide-y divide-border overflow-auto rounded-md border border-border">
        <PickList
          itemType={itemType}
          search={q}
          onPick={(id, title) => {
            onPick(id, title);
            onClose();
          }}
        />
      </div>
    </Modal>
  );
}

interface ListProps {
  itemType: EmbedItemType;
  search: string;
  onPick: (id: string, title: string) => void;
}

// Only the component matching `itemType` is mounted, so just one list query runs.
function PickList({ itemType, search, onPick }: ListProps) {
  switch (itemType) {
    case 'idea':
      return <IdeaList search={search} onPick={onPick} />;
    case 'document':
      return <DocumentList search={search} onPick={onPick} />;
    case 'page':
      return <PageList search={search} onPick={onPick} />;
    case 'file':
      return <FileList search={search} onPick={onPick} />;
    case 'mindmap':
      return <MindmapList search={search} onPick={onPick} />;
    case 'diagram':
      return <DiagramList search={search} onPick={onPick} />;
    default:
      return null;
  }
}

function Rows({
  isLoading,
  isError,
  items,
  onPick,
}: {
  isLoading: boolean;
  isError: boolean;
  items: { id: string; title: string; subtitle?: string }[];
  onPick: (id: string, title: string) => void;
}) {
  if (isLoading)
    return (
      <div className="grid place-items-center p-6">
        <Spinner label="Loading" />
      </div>
    );
  if (isError) return <p className="p-4 text-sm text-danger">Couldn’t load items.</p>;
  if (items.length === 0)
    return <p className="p-4 text-sm text-muted-foreground">No matching items.</p>;
  return (
    <ul>
      {items.map((it) => (
        <li key={it.id}>
          <button
            type="button"
            onClick={() => onPick(it.id, it.title)}
            className={cn(
              'flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left text-sm',
              'hover:bg-muted focus:bg-muted focus:outline-none',
            )}
          >
            <span className="font-medium">{it.title || 'Untitled'}</span>
            {it.subtitle && (
              <span className="text-xs text-muted-foreground">{it.subtitle}</span>
            )}
          </button>
        </li>
      ))}
    </ul>
  );
}

function IdeaList({ search, onPick }: { search: string; onPick: ListProps['onPick'] }) {
  const { data = [], isLoading, isError } = useIdeas({ status: null, tag: null, q: search });
  return (
    <Rows
      isLoading={isLoading}
      isError={isError}
      onPick={onPick}
      items={data.map((i) => ({ id: i.id, title: i.title, subtitle: i.status }))}
    />
  );
}

function DocumentList({ search, onPick }: { search: string; onPick: ListProps['onPick'] }) {
  const { data = [], isLoading, isError } = useDocuments();
  const q = search.toLowerCase();
  const items = data
    .filter((d) => !q || d.title.toLowerCase().includes(q))
    .map((d) => ({ id: d.id, title: d.title }));
  return <Rows isLoading={isLoading} isError={isError} onPick={onPick} items={items} />;
}

function PageList({ search, onPick }: { search: string; onPick: ListProps['onPick'] }) {
  const { data = [], isLoading, isError } = usePages({ q: search || undefined });
  return (
    <Rows
      isLoading={isLoading}
      isError={isError}
      onPick={onPick}
      items={data.map((p) => ({ id: p.id, title: p.title, subtitle: p.visibility }))}
    />
  );
}

function FileList({ search, onPick }: { search: string; onPick: ListProps['onPick'] }) {
  const { data, isLoading, isError } = useListing(null, search);
  const items = (data?.files ?? []).map((f) => ({ id: f.id, title: f.name, subtitle: f.mime }));
  return <Rows isLoading={isLoading} isError={isError} onPick={onPick} items={items} />;
}

function MindmapList({ search, onPick }: { search: string; onPick: ListProps['onPick'] }) {
  const { data = [], isLoading, isError } = useMindmaps({ q: search || undefined });
  return (
    <Rows
      isLoading={isLoading}
      isError={isError}
      onPick={onPick}
      items={data.map((m) => ({ id: m.id, title: m.title }))}
    />
  );
}

function DiagramList({ search, onPick }: { search: string; onPick: ListProps['onPick'] }) {
  const { data = [], isLoading, isError } = useDiagrams({ q: search || undefined });
  return (
    <Rows
      isLoading={isLoading}
      isError={isError}
      onPick={onPick}
      items={data.map((d) => ({ id: d.id, title: d.title }))}
    />
  );
}
