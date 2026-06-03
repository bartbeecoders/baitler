import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';

import {
  useCreateDocument,
  useDeleteDocument,
  useDocuments,
} from '@/features/documents/api';
import { useCreateIdea, useDeleteIdea, useIdeas, useTags } from '@/features/ideas/api';
import { STATUSES, STATUS_LABELS, type IdeaStatus } from '@/features/ideas/types';
import {
  useCreatePage,
  useDeletePage,
  useFolders as usePageFolders,
  usePages,
} from '@/features/pages/api';
import { useCreateMindmap, useDeleteMindmap, useFolders as useMindmapFolders, useMindmaps } from '@/features/mindmaps/api';
import { useCreateDiagram, useDeleteDiagram, useDiagrams, useFolders as useDiagramFolders } from '@/features/diagrams/api';
import { VISIBILITIES } from '@/features/pages/types';
import { ObjectList, type ObjectListItem } from './ObjectList';

/** Debounce a fast-changing value (search box) before it hits a server query. */
function useDebounced<T>(value: T, ms = 250): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return debounced;
}

/** The selected item id parsed from the URL (null on the base/`new` route). */
function useActiveId(base: string): string | null {
  const { pathname } = useLocation();
  if (pathname === base || !pathname.startsWith(`${base}/`)) return null;
  const rest = pathname.slice(base.length + 1).split('/')[0];
  return !rest || rest === 'new' ? null : rest;
}

// ── Documents ───────────────────────────────────────────────────────────────

export function DocumentsList() {
  const nav = useNavigate();
  const qc = useQueryClient();
  const active = useActiveId('/editor');
  const [search, setSearch] = useState('');
  const [tag, setTag] = useState('');
  const { data: docs = [], isLoading, isError } = useDocuments();
  const create = useCreateDocument();
  const remove = useDeleteDocument();

  const tags = useMemo(
    () => Array.from(new Set(docs.flatMap((d) => d.tags ?? []))).sort(),
    [docs],
  );
  const items: ObjectListItem[] = docs
    .filter((d) => !tag || (d.tags ?? []).includes(tag))
    .filter((d) => !search || d.title.toLowerCase().includes(search.toLowerCase()))
    .map((d) => ({ id: d.id, title: d.title }));

  return (
    <ObjectList
      noun="document"
      items={items}
      isLoading={isLoading}
      isError={isError}
      activeId={active}
      search={search}
      onSearch={setSearch}
      filters={[
        {
          key: 'tag',
          label: 'All tags',
          value: tag,
          onChange: setTag,
          options: tags.map((t) => ({ value: t, label: t })),
        },
      ]}
      onSelect={(id) => nav(`/editor/${id}`)}
      onCreate={() =>
        create.mutate(
          { title: 'Untitled', body: '<p></p>' },
          { onSuccess: (d) => nav(`/editor/${d.id}`) },
        )
      }
      creating={create.isPending}
      onDelete={(id) =>
        remove.mutate(id, { onSuccess: () => active === id && nav('/editor') })
      }
      deleting={remove.isPending}
      onRefresh={() => qc.invalidateQueries({ queryKey: ['documents'] })}
    />
  );
}

// ── Ideas ─────────────────────────────────────────────────────────────────────

export function IdeasList() {
  const nav = useNavigate();
  const qc = useQueryClient();
  const active = useActiveId('/ideas');
  const [search, setSearch] = useState('');
  const [status, setStatus] = useState('');
  const [tag, setTag] = useState('');
  const q = useDebounced(search.trim());
  const {
    data: ideas = [],
    isLoading,
    isError,
  } = useIdeas({ status: (status || null) as IdeaStatus | null, tag: tag || null, q });
  const { data: tags = [] } = useTags();
  const remove = useDeleteIdea();
  // Created via the modal (ideas require a title), so the + routes to /ideas/new.
  useCreateIdea();

  const items: ObjectListItem[] = ideas.map((i) => ({
    id: i.id,
    title: i.title,
    badge: { label: STATUS_LABELS[i.status], variant: 'muted' },
  }));

  return (
    <ObjectList
      noun="idea"
      items={items}
      isLoading={isLoading}
      isError={isError}
      activeId={active}
      search={search}
      onSearch={setSearch}
      filters={[
        {
          key: 'status',
          label: 'All statuses',
          value: status,
          onChange: setStatus,
          options: STATUSES.map((s) => ({ value: s, label: STATUS_LABELS[s] })),
        },
        {
          key: 'tag',
          label: 'All tags',
          value: tag,
          onChange: setTag,
          options: tags.map((t) => ({ value: t, label: t })),
        },
      ]}
      onSelect={(id) => nav(`/ideas/${id}`)}
      onCreate={() => nav('/ideas/new')}
      onDelete={(id) =>
        remove.mutate(id, { onSuccess: () => active === id && nav('/ideas') })
      }
      deleting={remove.isPending}
      onRefresh={() => qc.invalidateQueries({ queryKey: ['ideas'] })}
    />
  );
}

// ── Pages ─────────────────────────────────────────────────────────────────────

const VISIBILITY_LABELS: Record<string, string> = {
  draft: 'Draft',
  unlisted: 'Unlisted',
  public: 'Public',
};

export function PagesList() {
  const nav = useNavigate();
  const qc = useQueryClient();
  const active = useActiveId('/pages');
  const [search, setSearch] = useState('');
  const [visibility, setVisibility] = useState('');
  const [folder, setFolder] = useState('');
  const q = useDebounced(search.trim());
  const {
    data: pages = [],
    isLoading,
    isError,
  } = usePages({
    q: q || undefined,
    visibility: (visibility || undefined) as never,
    folder: folder || undefined,
  });
  const { data: folders = [] } = usePageFolders();
  const create = useCreatePage();
  const remove = useDeletePage();

  const items: ObjectListItem[] = pages.map((p) => ({
    id: p.id,
    title: p.title,
    badge: {
      label: VISIBILITY_LABELS[p.visibility] ?? p.visibility,
      variant: p.visibility === 'public' ? 'success' : p.visibility === 'unlisted' ? 'warning' : 'muted',
    },
  }));

  return (
    <ObjectList
      noun="page"
      items={items}
      isLoading={isLoading}
      isError={isError}
      activeId={active}
      search={search}
      onSearch={setSearch}
      filters={[
        {
          key: 'visibility',
          label: 'All visibility',
          value: visibility,
          onChange: setVisibility,
          options: VISIBILITIES.map((v) => ({ value: v, label: VISIBILITY_LABELS[v] ?? v })),
        },
        {
          key: 'folder',
          label: 'All folders',
          value: folder,
          onChange: setFolder,
          options: folders.map((f) => ({ value: f.id, label: f.name })),
        },
      ]}
      onSelect={(id) => nav(`/pages/${id}`)}
      onCreate={() =>
        create.mutate(
          { title: 'Untitled', body: '<p></p>' },
          { onSuccess: (p) => nav(`/pages/${p.id}`) },
        )
      }
      creating={create.isPending}
      onDelete={(id) => remove.mutate(id, { onSuccess: () => active === id && nav('/pages') })}
      deleting={remove.isPending}
      onRefresh={() => qc.invalidateQueries({ queryKey: ['pages'] })}
    />
  );
}

// ── Mindmaps ────────────────────────────────────────────────────────────────

export function MindmapsList() {
  const nav = useNavigate();
  const qc = useQueryClient();
  const active = useActiveId('/mindmaps');
  const [search, setSearch] = useState('');
  const [folder, setFolder] = useState('');
  const q = useDebounced(search.trim());
  const { data: mindmaps = [], isLoading, isError } = useMindmaps({
    q: q || undefined,
    folder: folder || undefined,
  });
  const { data: folders = [] } = useMindmapFolders();
  const create = useCreateMindmap();
  const remove = useDeleteMindmap();

  const items: ObjectListItem[] = mindmaps.map((m) => ({
    id: m.id,
    title: m.title,
    badge: m.review === 'draft' ? { label: 'Draft', variant: 'warning' } : undefined,
  }));

  return (
    <ObjectList
      noun="mindmap"
      items={items}
      isLoading={isLoading}
      isError={isError}
      activeId={active}
      search={search}
      onSearch={setSearch}
      filters={[
        {
          key: 'folder',
          label: 'All folders',
          value: folder,
          onChange: setFolder,
          options: folders.map((f) => ({ value: f.id, label: f.name })),
        },
      ]}
      onSelect={(id) => nav(`/mindmaps/${id}`)}
      onCreate={() =>
        create.mutate(
          { title: 'Untitled map', graph: { nodes: [{ id: 'n1', label: 'Idea' }], edges: [] } },
          { onSuccess: (m) => nav(`/mindmaps/${m.id}`) },
        )
      }
      creating={create.isPending}
      onDelete={(id) => remove.mutate(id, { onSuccess: () => active === id && nav('/mindmaps') })}
      deleting={remove.isPending}
      onRefresh={() => qc.invalidateQueries({ queryKey: ['mindmaps'] })}
    />
  );
}

// ── Diagrams ──────────────────────────────────────────────────────────────────

export function DiagramsList() {
  const nav = useNavigate();
  const qc = useQueryClient();
  const active = useActiveId('/diagrams');
  const [search, setSearch] = useState('');
  const [folder, setFolder] = useState('');
  const q = useDebounced(search.trim());
  const { data: diagrams = [], isLoading, isError } = useDiagrams({
    q: q || undefined,
    folder: folder || undefined,
  });
  const { data: folders = [] } = useDiagramFolders();
  const create = useCreateDiagram();
  const remove = useDeleteDiagram();

  const items: ObjectListItem[] = diagrams.map((d) => ({
    id: d.id,
    title: d.title,
    badge: d.review === 'draft' ? { label: 'Draft', variant: 'warning' } : undefined,
  }));

  return (
    <ObjectList
      noun="diagram"
      items={items}
      isLoading={isLoading}
      isError={isError}
      activeId={active}
      search={search}
      onSearch={setSearch}
      filters={[
        {
          key: 'folder',
          label: 'All folders',
          value: folder,
          onChange: setFolder,
          options: folders.map((f) => ({ value: f.id, label: f.name })),
        },
      ]}
      onSelect={(id) => nav(`/diagrams/${id}`)}
      onCreate={() =>
        create.mutate(
          { title: 'Untitled diagram' },
          { onSuccess: (d) => nav(`/diagrams/${d.id}`) },
        )
      }
      creating={create.isPending}
      onDelete={(id) => remove.mutate(id, { onSuccess: () => active === id && nav('/diagrams') })}
      deleting={remove.isPending}
      onRefresh={() => qc.invalidateQueries({ queryKey: ['diagrams'] })}
    />
  );
}
