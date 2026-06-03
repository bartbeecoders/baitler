import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { ExternalLink, Plus, Trash2 } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { ConfirmModal } from '@/components/ui/confirm-modal';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { TagInput } from '@/features/ideas/TagInput';
import { EmptyDetail } from '@/features/objects/EmptyDetail';
import { OBJECT_CANVAS, OBJECT_EDITOR_ROOT } from '@/features/objects/editorLayout';
import { apiFetch } from '@/lib/api';
import { cn } from '@/lib/cn';
import {
  errorMessage,
  useCreateSuperpageFromProject,
  useDeleteSuperpage,
  useProjects,
  useSuperpage,
  useUpdateSuperpage,
} from './api';
import type { EmbedItemType, Superpage, SuperpageBlock, SuperpageLayout } from './types';

const EMBED_ROUTES: Record<EmbedItemType, string> = {
  idea: '/ideas',
  document: '/editor',
  file: '/files',
  page: '/pages',
  mindmap: '/mindmaps',
  diagram: '/diagrams',
};

function newId() {
  return `b${Date.now().toString(36)}`;
}

function normalizeLayout(raw: SuperpageLayout | undefined): SuperpageLayout {
  return {
    layout: raw?.layout ?? 'grid',
    blocks: raw?.blocks ?? [],
  };
}

interface ResolvedPayload {
  title?: string;
  missing?: boolean;
}

interface ContextBlock {
  id: string;
  kind: string;
  payload: ResolvedPayload & Record<string, unknown>;
}

export function SuperpagesPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const detail = useSuperpage(id ?? null);

  if (!id) return <EmptyDetail noun="superpage" />;
  if (detail.isLoading) {
    return (
      <div className="grid h-[calc(100svh-12rem)] place-items-center">
        <Spinner label="Loading superpage" />
      </div>
    );
  }
  if (detail.isError) {
    return (
      <p className="text-sm text-danger" role="alert">
        {errorMessage(detail.error)}
      </p>
    );
  }
  if (!detail.data) return <EmptyDetail noun="superpage" />;

  return (
    <SuperpageEditor
      key={detail.data.id}
      superpage={detail.data}
      onDeleted={() => navigate('/superpages')}
    />
  );
}

function SuperpageEditor({
  superpage,
  onDeleted,
}: {
  superpage: Superpage;
  onDeleted: () => void;
}) {
  const navigate = useNavigate();
  const update = useUpdateSuperpage(superpage.id);
  const remove = useDeleteSuperpage();
  const fromProject = useCreateSuperpageFromProject();
  const projects = useProjects();

  const [title, setTitle] = useState(superpage.title);
  const [tags, setTags] = useState(superpage.tags ?? []);
  const [layout, setLayout] = useState<SuperpageLayout>(() => normalizeLayout(superpage.blocks));
  const [resolved, setResolved] = useState<Record<string, ResolvedPayload>>({});
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    setTitle(superpage.title);
    setTags(superpage.tags ?? []);
    setLayout(normalizeLayout(superpage.blocks));
    setDirty(false);
  }, [superpage.id, superpage.updated_at]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const ctx = await apiFetch<{ blocks: ContextBlock[] }>(
          `/superpages/${superpage.id}/context`,
        );
        if (cancelled) return;
        const map: Record<string, ResolvedPayload> = {};
        for (const b of ctx.blocks) {
          map[b.id] = b.payload;
        }
        setResolved(map);
      } catch {
        /* context is best-effort for labels */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [superpage.id, superpage.updated_at]);

  const persist = useCallback(() => {
    update.mutate({ title, tags, blocks: layout });
    setDirty(false);
  }, [layout, tags, title, update]);

  useEffect(() => {
    if (!dirty) return;
    const t = setTimeout(persist, 800);
    return () => clearTimeout(t);
  }, [dirty, persist]);

  const addBlock = (kind: SuperpageBlock['kind']) => {
    const block: SuperpageBlock = {
      id: newId(),
      kind,
      w: 6,
      h: 3,
      ...(kind === 'note' ? { markdown: '' } : {}),
      ...(kind === 'heading' ? { text: 'Section' } : {}),
    };
    setLayout((l) => ({ ...l, blocks: [...l.blocks, block] }));
    setDirty(true);
  };

  const updateBlock = (id: string, patch: Partial<SuperpageBlock>) => {
    setLayout((l) => ({
      ...l,
      blocks: l.blocks.map((b) => (b.id === id ? { ...b, ...patch } : b)),
    }));
    setDirty(true);
  };

  const removeBlock = (id: string) => {
    setLayout((l) => ({ ...l, blocks: l.blocks.filter((b) => b.id !== id) }));
    setDirty(true);
  };

  const openEmbed = (b: SuperpageBlock) => {
    if (!b.item_type || !b.item_id) return;
    const base = EMBED_ROUTES[b.item_type as EmbedItemType];
    if (base) navigate(`${base}/${b.item_id}`);
  };

  const gridStyle = useMemo(
    () => ({
      display: 'grid',
      gridTemplateColumns: 'repeat(12, minmax(0, 1fr))',
      gap: '0.75rem',
      alignContent: 'start',
    }),
    [],
  );

  return (
    <div className={OBJECT_EDITOR_ROOT} data-testid="superpage-editor">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={title}
          onChange={(e) => {
            setTitle(e.target.value);
            setDirty(true);
          }}
          className="max-w-md font-semibold"
          aria-label="Superpage title"
        />
        <Button type="button" size="sm" variant="secondary" onClick={() => addBlock('heading')}>
          <Plus className="h-4 w-4" aria-hidden />
          Heading
        </Button>
        <Button type="button" size="sm" variant="secondary" onClick={() => addBlock('note')}>
          <Plus className="h-4 w-4" aria-hidden />
          Note
        </Button>
        <Button type="button" size="sm" variant="secondary" onClick={() => addBlock('embed')}>
          <Plus className="h-4 w-4" aria-hidden />
          Embed
        </Button>
        <select
          className="rounded-md border border-border bg-background px-2 py-1 text-sm"
          defaultValue=""
          onChange={(e) => {
            const pid = e.target.value;
            if (!pid) return;
            fromProject.mutate(
              { project_id: pid },
              { onSuccess: (sp) => navigate(`/superpages/${sp.id}`) },
            );
            e.target.value = '';
          }}
          aria-label="Seed from project"
        >
          <option value="">Seed from project…</option>
          {(projects.data ?? []).map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <div className="flex-1" />
        <Button type="button" size="sm" variant="ghost" onClick={() => setConfirmDelete(true)}>
          <Trash2 className="h-4 w-4" aria-hidden />
          Delete
        </Button>
      </div>

      <TagInput tags={tags} onChange={(t) => { setTags(t); setDirty(true); }} />

      <div className={OBJECT_CANVAS} data-testid="superpage-canvas">
        <div className="absolute inset-0 overflow-auto p-4">
          <div style={gridStyle}>
            {layout.blocks.map((block) => {
              const col = block.w ?? 6;
              const payload = resolved[block.id];
              return (
                <div
                  key={block.id}
                  className={cn(
                    'rounded-lg border border-border bg-card p-3 shadow-sm',
                    block.kind === 'heading' && 'bg-muted/40',
                  )}
                  style={{ gridColumn: `span ${Math.min(12, Math.max(2, col))}` }}
                >
                  <div className="mb-2 flex items-center justify-between gap-2">
                    <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                      {block.kind}
                    </span>
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      onClick={() => removeBlock(block.id)}
                      aria-label="Remove block"
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  </div>

                  {block.kind === 'heading' && (
                    <Input
                      value={block.text ?? ''}
                      onChange={(e) => updateBlock(block.id, { text: e.target.value })}
                      className="font-semibold"
                    />
                  )}

                  {block.kind === 'note' && (
                    <textarea
                      className="min-h-[6rem] w-full rounded-md border border-border bg-background p-2 text-sm"
                      value={block.markdown ?? ''}
                      onChange={(e) => updateBlock(block.id, { markdown: e.target.value })}
                      placeholder="Notes for you or the agent…"
                    />
                  )}

                  {block.kind === 'embed' && (
                    <div className="space-y-2 text-sm">
                      <div className="flex flex-wrap gap-2">
                        <select
                          className="rounded border border-border bg-background px-2 py-1"
                          value={block.item_type ?? 'idea'}
                          onChange={(e) =>
                            updateBlock(block.id, {
                              item_type: e.target.value as EmbedItemType,
                              item_id: '',
                            })
                          }
                        >
                          {(Object.keys(EMBED_ROUTES) as EmbedItemType[]).map((t) => (
                            <option key={t} value={t}>
                              {t}
                            </option>
                          ))}
                        </select>
                        <Input
                          placeholder="Item id"
                          value={block.item_id ?? ''}
                          onChange={(e) => updateBlock(block.id, { item_id: e.target.value })}
                          className="min-w-[12rem] flex-1 font-mono text-xs"
                        />
                      </div>
                      <p className="text-muted-foreground">
                        {payload?.missing
                          ? 'Item not found'
                          : payload?.title ?? 'Save to resolve title'}
                      </p>
                      {block.item_id && (
                        <Button
                          type="button"
                          size="sm"
                          variant="secondary"
                          onClick={() => openEmbed(block)}
                        >
                          <ExternalLink className="h-3 w-3" aria-hidden />
                          Open
                        </Button>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
          {layout.blocks.length === 0 && (
            <p className="text-sm text-muted-foreground">
              Add blocks to compose your board — embeds reference existing Baitler content.
            </p>
          )}
        </div>
      </div>

      {update.isPending && (
        <p className="text-xs text-muted-foreground" aria-live="polite">
          Saving…
        </p>
      )}

      <ConfirmModal
        open={confirmDelete}
        title="Delete superpage?"
        message="This removes the board; embedded items are not deleted."
        confirmLabel="Delete"
        danger
        pending={remove.isPending}
        onConfirm={() =>
          remove.mutate(superpage.id, {
            onSuccess: () => {
              setConfirmDelete(false);
              onDeleted();
            },
          })
        }
        onClose={() => setConfirmDelete(false)}
      />
    </div>
  );
}