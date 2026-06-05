import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  Code2,
  ExternalLink,
  Eye,
  Globe,
  GripVertical,
  Image as ImageIcon,
  type LucideIcon,
  Network,
  Paperclip,
  Pencil,
  Plus,
  Sparkles,
  Trash2,
  Type,
  Workflow,
} from 'lucide-react';

import { Button } from '@/components/ui/button';
import { ConfirmModal } from '@/components/ui/confirm-modal';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { TagInput } from '@/features/ideas/TagInput';
import { useDownload, useUpload } from '@/features/files/api';
import { EmptyDetail } from '@/features/objects/EmptyDetail';
import { OBJECT_CANVAS, OBJECT_EDITOR_ROOT } from '@/features/objects/editorLayout';
import { cn } from '@/lib/cn';
import { CanvasAiPane } from './CanvasAiPane';
import { EmbedPicker } from './EmbedPicker';
import { PartContent, type PartActions } from './PartContent';
import { useCreateEmbedItem } from './useCreateEmbedItem';
import {
  errorMessage,
  useDeleteSuperpage,
  useGenerateImage,
  useSuperpage,
  useSuperpageContext,
  useUpdateSuperpage,
} from './api';
import {
  PART_KINDS,
  PART_LABELS,
  type EmbedItemType,
  type PartKind,
  type ResolvedBlockPayload,
  type Superpage,
  type SuperpageBlock,
  type SuperpageLayout,
} from './types';

const PART_ICONS: Record<PartKind, LucideIcon> = {
  text: Type,
  image: ImageIcon,
  code: Code2,
  file: Paperclip,
  webpage: Globe,
  mindmap: Network,
  diagram: Workflow,
};

const EMBED_ROUTES: Record<EmbedItemType, string> = {
  idea: '/ideas',
  document: '/editor',
  file: '/files',
  page: '/pages',
  mindmap: '/mindmaps',
  diagram: '/diagrams',
};

const SNAP = 8;
const MIN_W = 180;
const MIN_H = 100;
const PAD = 16;
const snap = (v: number) => Math.round(v / SNAP) * SNAP;

function newId() {
  return `b${Date.now().toString(36)}${Math.floor(Math.random() * 1e4).toString(36)}`;
}

function defaultGeometry(kind: SuperpageBlock['kind']): { w: number; h: number } {
  switch (kind) {
    case 'text':
      return { w: 360, h: 220 };
    case 'code':
      return { w: 380, h: 220 };
    case 'image':
      return { w: 320, h: 260 };
    case 'file':
      return { w: 320, h: 120 };
    case 'webpage':
      return { w: 440, h: 320 };
    default:
      return { w: 360, h: 260 };
  }
}

function newPart(kind: PartKind, spot: { x: number; y: number }): SuperpageBlock {
  const geo = defaultGeometry(kind);
  const base: SuperpageBlock = { id: newId(), kind, ...spot, ...geo };
  if (kind === 'text') base.markdown = '';
  if (kind === 'code') {
    base.text = '';
    base.lang = '';
  }
  if (kind === 'image') {
    base.src = 'url';
    base.url = '';
  }
  if (kind === 'webpage') {
    base.web_kind = 'url';
    base.url = '';
  }
  return base;
}

/** Map legacy v1 blocks onto the new typed parts. */
function migrateBlock(b: SuperpageBlock): SuperpageBlock {
  if (b.kind === 'note') return { ...b, kind: 'text', markdown: b.markdown ?? '' };
  if (b.kind === 'heading')
    return { ...b, kind: 'text', markdown: b.text ? `## ${b.text}` : '', text: undefined };
  if (b.kind === 'embed') {
    switch (b.item_type) {
      case 'file':
        return { ...b, kind: 'file' };
      case 'page':
        return { ...b, kind: 'webpage', web_kind: 'page' };
      case 'mindmap':
        return { ...b, kind: 'mindmap' };
      case 'diagram':
        return { ...b, kind: 'diagram' };
      default:
        return b; // idea/document stay as legacy embed (read-only)
    }
  }
  return b;
}

function autoFlow(blocks: SuperpageBlock[]): SuperpageBlock[] {
  const COLS = 3;
  const CW = 400;
  const RH = 300;
  return blocks.map((b, i) => {
    const geo = defaultGeometry(b.kind);
    return {
      ...b,
      x: PAD + (i % COLS) * CW,
      y: PAD + Math.floor(i / COLS) * RH,
      w: b.w && b.w >= 40 ? b.w : geo.w,
      h: b.h && b.h >= 40 ? b.h : geo.h,
    };
  });
}

function ensureCanvas(raw: SuperpageLayout | undefined): SuperpageLayout {
  const migrated = (raw?.blocks ?? []).map(migrateBlock);
  const placed = migrated.every(
    (b) => typeof b.x === 'number' && typeof b.y === 'number' && (b.w ?? 0) >= 40 && (b.h ?? 0) >= 40,
  );
  const blocks = raw?.layout === 'canvas' && placed ? migrated : autoFlow(migrated);
  return { layout: 'canvas', blocks };
}

function nextSpot(blocks: SuperpageBlock[]): { x: number; y: number } {
  if (blocks.length === 0) return { x: PAD, y: PAD };
  const bottom = Math.max(...blocks.map((b) => (b.y ?? 0) + (b.h ?? 0)));
  return { x: PAD, y: snap(bottom + PAD) };
}

const isSafeUrl = (url: string | undefined, allowData: boolean) => {
  const u = (url ?? '').trim().toLowerCase();
  return u.startsWith('https://') || u.startsWith('http://') || (allowData && u.startsWith('data:'));
};

/** A part is persisted only when complete — mirrors the backend validator so
 *  autosave never 400s on a part the user is still composing. */
function isComplete(b: SuperpageBlock): boolean {
  switch (b.kind) {
    case 'image':
      return b.src === 'upload' ? Boolean(b.item_id) : isSafeUrl(b.url, true);
    case 'file':
    case 'mindmap':
    case 'diagram':
      return Boolean(b.item_id);
    case 'webpage':
      return b.web_kind === 'page' ? Boolean(b.item_id) : isSafeUrl(b.url, false);
    case 'embed':
      return Boolean(b.item_type && b.item_id);
    default:
      return true; // text / code / note / heading
  }
}

function sanitizeForSave(layout: SuperpageLayout): SuperpageLayout {
  return { ...layout, blocks: layout.blocks.filter(isComplete) };
}

function stripHtml(html?: string): string {
  return (html ?? '').replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
}

/** Plain-text rendering of one part for AI grounding. */
function partToContext(block: SuperpageBlock, payload?: ResolvedBlockPayload): string {
  switch (block.kind) {
    case 'text':
    case 'note':
      return block.markdown ?? '';
    case 'code':
      return `\`\`\`${block.lang ?? ''}\n${block.text ?? ''}\n\`\`\``;
    case 'image':
      return `[image${block.text ? `: ${block.text}` : ''}]`;
    case 'file':
      return `[file: ${payload?.name ?? payload?.title ?? block.item_id ?? ''}]`;
    case 'webpage':
      return block.web_kind === 'page'
        ? `[page: ${payload?.title ?? ''}]\n${stripHtml(payload?.body).slice(0, 2000)}`
        : `[web page: ${block.url ?? ''}]`;
    case 'mindmap':
      return `[mindmap: ${payload?.title ?? ''}] nodes: ${(payload?.graph?.nodes ?? [])
        .map((n) => n.label)
        .join(', ')}`;
    case 'diagram':
      return `[diagram: ${payload?.title ?? ''}]`;
    case 'heading':
      return `# ${block.text ?? ''}`;
    case 'embed':
      return `[${block.item_type}: ${payload?.title ?? ''}] ${stripHtml(payload?.body).slice(0, 1500)}`;
    default:
      return '';
  }
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

interface PickTarget {
  blockId: string;
  type: EmbedItemType;
}

function SuperpageEditor({ superpage, onDeleted }: { superpage: Superpage; onDeleted: () => void }) {
  const navigate = useNavigate();
  const update = useUpdateSuperpage(superpage.id);
  const remove = useDeleteSuperpage();
  const context = useSuperpageContext(superpage.id, superpage.version);
  const { create: createItem } = useCreateEmbedItem();
  const upload = useUpload();
  const generate = useGenerateImage();
  const download = useDownload();

  const [title, setTitle] = useState(superpage.title);
  const [tags, setTags] = useState(superpage.tags ?? []);
  const [layout, setLayout] = useState<SuperpageLayout>(() => ensureCanvas(superpage.blocks));
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [mode, setMode] = useState<'edit' | 'view'>('edit');
  const [pickFor, setPickFor] = useState<PickTarget | null>(null);
  const [busyIds, setBusyIds] = useState<Record<string, boolean>>({});
  const [aiOpen, setAiOpen] = useState(false);
  const [aiTargetId, setAiTargetId] = useState<string | null>(null);

  const resolved = useMemo(() => {
    const map: Record<string, ResolvedBlockPayload> = {};
    for (const b of context.data ?? []) map[b.id] = b;
    return map;
  }, [context.data]);

  const persist = useCallback(() => {
    update.mutate({ title, tags, blocks: sanitizeForSave(layout) });
    setDirty(false);
  }, [layout, tags, title, update]);

  useEffect(() => {
    if (!dirty) return;
    const t = setTimeout(persist, 800);
    return () => clearTimeout(t);
  }, [dirty, persist]);

  const updateBlock = useCallback((blockId: string, patch: Partial<SuperpageBlock>) => {
    setLayout((l) => ({
      ...l,
      blocks: l.blocks.map((b) => (b.id === blockId ? { ...b, ...patch } : b)),
    }));
    setDirty(true);
  }, []);

  const addPart = (kind: PartKind) => {
    setLayout((l) => ({ ...l, blocks: [...l.blocks, newPart(kind, nextSpot(l.blocks))] }));
    setDirty(true);
  };

  const removeBlock = (blockId: string) => {
    setLayout((l) => ({ ...l, blocks: l.blocks.filter((b) => b.id !== blockId) }));
    if (aiTargetId === blockId) setAiTargetId(null);
    setDirty(true);
  };

  const setGeometry = (blockId: string, patch: Partial<SuperpageBlock>) =>
    updateBlock(blockId, patch);

  const runBusy = async (blockId: string, fn: () => Promise<void>) => {
    setBusyIds((s) => ({ ...s, [blockId]: true }));
    try {
      await fn();
    } finally {
      setBusyIds((s) => {
        const next = { ...s };
        delete next[blockId];
        return next;
      });
    }
  };

  const uploadFile = async (file: File): Promise<string | undefined> => {
    const res = await upload.mutateAsync({ files: [file], folderId: null });
    return (res as { files: { id: string }[] }).files?.[0]?.id;
  };

  const openItem = (block: SuperpageBlock) => {
    if (block.kind === 'file' && block.item_id) {
      download.mutate({ id: block.item_id, name: resolved[block.id]?.name ?? undefined });
      return;
    }
    if (block.kind === 'mindmap' && block.item_id) navigate(`/mindmaps/${block.item_id}`);
    else if (block.kind === 'diagram' && block.item_id) navigate(`/diagrams/${block.item_id}`);
    else if (block.kind === 'webpage' && block.web_kind === 'page' && block.item_id)
      navigate(`/pages/${block.item_id}`);
    else if (block.kind === 'embed' && block.item_type && block.item_id)
      navigate(`${EMBED_ROUTES[block.item_type]}/${block.item_id}`);
  };

  const actionsFor = (block: SuperpageBlock): PartActions => ({
    busy: Boolean(busyIds[block.id]),
    pick: (type) => setPickFor({ blockId: block.id, type }),
    open: () => openItem(block),
    download: () =>
      block.item_id && download.mutate({ id: block.item_id, name: resolved[block.id]?.name ?? undefined }),
    upload: (file, asImage) =>
      runBusy(block.id, async () => {
        const fileId = await uploadFile(file);
        if (fileId) updateBlock(block.id, asImage ? { src: 'upload', item_id: fileId } : { item_id: fileId });
      }),
    generateImage: (prompt) =>
      runBusy(block.id, async () => {
        const img = await generate.mutateAsync({ prompt });
        // Small (e.g. mock SVG) data-URLs inline cleanly; larger raster outputs
        // would blow the url length cap, so persist those as a file instead.
        if (img.data_url.length <= 3500) {
          updateBlock(block.id, { src: 'generated', url: img.data_url, item_id: undefined });
        } else {
          const blob = await (await fetch(img.data_url)).blob();
          const ext = img.mime.split('/')[1]?.split('+')[0] ?? 'png';
          const fileId = await uploadFile(new File([blob], `generated.${ext}`, { type: img.mime }));
          if (fileId) updateBlock(block.id, { src: 'upload', item_id: fileId, url: undefined });
        }
      }),
    createItem: (type) =>
      runBusy(block.id, async () => {
        const itemId = await createItem(type);
        if (itemId) updateBlock(block.id, type === 'page' ? { web_kind: 'page', item_id: itemId } : { item_id: itemId });
      }),
  });

  const extent = useMemo(() => {
    let w = 640;
    let h = 360;
    for (const b of layout.blocks) {
      w = Math.max(w, (b.x ?? 0) + (b.w ?? 0) + PAD * 4);
      h = Math.max(h, (b.y ?? 0) + (b.h ?? 0) + PAD * 6);
    }
    return { w, h };
  }, [layout.blocks]);

  const aiTarget = aiTargetId ? layout.blocks.find((b) => b.id === aiTargetId) : undefined;
  const aiContext = aiTarget
    ? partToContext(aiTarget, resolved[aiTarget.id])
    : layout.blocks
        .map((b) => partToContext(b, resolved[b.id]))
        .filter(Boolean)
        .join('\n\n')
        .slice(0, 16000);
  const aiCanApply = Boolean(aiTarget && (aiTarget.kind === 'text' || aiTarget.kind === 'code'));

  const applyAi = (text: string) => {
    if (!aiTarget) return;
    if (aiTarget.kind === 'text') updateBlock(aiTarget.id, { markdown: text });
    else if (aiTarget.kind === 'code') updateBlock(aiTarget.id, { text });
  };

  const openAi = (blockId: string | null) => {
    setAiTargetId(blockId);
    setAiOpen(true);
  };

  return (
    <div className={OBJECT_EDITOR_ROOT} data-testid="superpage-editor">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={title}
          onChange={(e) => {
            setTitle(e.target.value);
            setDirty(true);
          }}
          className="max-w-xs font-semibold"
          aria-label="Superpage title"
        />
        <Button
          type="button"
          size="sm"
          variant={mode === 'view' ? 'primary' : 'secondary'}
          onClick={() => setMode((m) => (m === 'edit' ? 'view' : 'edit'))}
          aria-pressed={mode === 'view'}
        >
          {mode === 'edit' ? (
            <>
              <Eye className="h-4 w-4" aria-hidden /> View
            </>
          ) : (
            <>
              <Pencil className="h-4 w-4" aria-hidden /> Edit
            </>
          )}
        </Button>
        {mode === 'edit' && <AddPartMenu onAdd={addPart} />}
        <Button type="button" size="sm" variant="secondary" onClick={() => openAi(null)}>
          <Sparkles className="h-4 w-4" aria-hidden /> Assistant
        </Button>

        <div className="flex-1" />
        {update.isPending ? (
          <span className="text-xs text-muted-foreground" aria-live="polite">
            Saving…
          </span>
        ) : update.isError ? (
          <span className="text-xs text-danger" role="alert">
            {errorMessage(update.error)}
          </span>
        ) : null}
        <Button type="button" size="sm" variant="ghost" onClick={() => setConfirmDelete(true)}>
          <Trash2 className="h-4 w-4" aria-hidden /> Delete
        </Button>
      </div>

      {mode === 'edit' && (
        <TagInput
          tags={tags}
          onChange={(t) => {
            setTags(t);
            setDirty(true);
          }}
        />
      )}

      <div className={OBJECT_CANVAS} data-testid="superpage-canvas">
        <div
          className={cn(
            'absolute inset-0 overflow-auto',
            mode === 'edit' &&
              '[background-image:radial-gradient(circle,var(--color-border)_1px,transparent_1px)] [background-size:24px_24px]',
          )}
        >
          <div className="relative" style={{ minWidth: extent.w, minHeight: extent.h }}>
            {layout.blocks.map((block) => (
              <CanvasBlock
                key={block.id}
                block={block}
                payload={resolved[block.id]}
                editing={mode === 'edit'}
                openable={isOpenable(block)}
                onUpdate={(patch) => updateBlock(block.id, patch)}
                onGeometry={(patch) => setGeometry(block.id, patch)}
                onRemove={() => removeBlock(block.id)}
                onOpen={() => openItem(block)}
                onAi={() => openAi(block.id)}
                actions={actionsFor(block)}
              />
            ))}
            {layout.blocks.length === 0 && (
              <p className="absolute left-4 top-4 max-w-md text-sm text-muted-foreground">
                Empty canvas. Use <strong>Add part</strong> to drop in text, images, code, files, web
                pages, mindmaps, or diagrams — then drag and resize them anywhere. The assistant can
                read and edit your parts.
              </p>
            )}
          </div>
        </div>
      </div>

      {pickFor && (
        <EmbedPicker
          open
          itemType={pickFor.type}
          onClose={() => setPickFor(null)}
          onPick={(itemId) =>
            updateBlock(pickFor.blockId, {
              item_id: itemId,
              ...(pickFor.type === 'page' ? { web_kind: 'page' as const } : {}),
            })
          }
        />
      )}

      <CanvasAiPane
        open={aiOpen}
        onClose={() => setAiOpen(false)}
        contextLabel={aiTarget ? `${PART_LABELS[aiTarget.kind as PartKind] ?? aiTarget.kind} part` : 'Whole canvas'}
        context={aiContext}
        canApply={aiCanApply}
        onApply={applyAi}
      />

      <ConfirmModal
        open={confirmDelete}
        title="Delete superpage?"
        message="This removes the canvas; referenced items are not deleted."
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

function isOpenable(block: SuperpageBlock): boolean {
  if (block.kind === 'file') return Boolean(block.item_id);
  if (block.kind === 'mindmap' || block.kind === 'diagram') return Boolean(block.item_id);
  if (block.kind === 'webpage') return block.web_kind === 'page' && Boolean(block.item_id);
  if (block.kind === 'embed') return Boolean(block.item_type && block.item_id);
  return false;
}

function AddPartMenu({ onAdd }: { onAdd: (kind: PartKind) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <Button type="button" size="sm" variant="secondary" onClick={() => setOpen((o) => !o)}>
        <Plus className="h-4 w-4" aria-hidden /> Add part
      </Button>
      {open && (
        <>
          <button
            type="button"
            aria-hidden
            tabIndex={-1}
            className="fixed inset-0 z-10 cursor-default"
            onClick={() => setOpen(false)}
          />
          <div className="absolute left-0 top-full z-20 mt-1 w-44 rounded-md border border-border bg-card p-1 shadow-lg">
            {PART_KINDS.map((kind) => {
              const Icon = PART_ICONS[kind];
              return (
                <button
                  key={kind}
                  type="button"
                  onClick={() => {
                    onAdd(kind);
                    setOpen(false);
                  }}
                  className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-muted"
                >
                  <Icon className="h-4 w-4 text-muted-foreground" aria-hidden />
                  {PART_LABELS[kind]}
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

function CanvasBlock({
  block,
  payload,
  editing,
  openable,
  onUpdate,
  onGeometry,
  onRemove,
  onOpen,
  onAi,
  actions,
}: {
  block: SuperpageBlock;
  payload?: ResolvedBlockPayload;
  editing: boolean;
  openable: boolean;
  onUpdate: (patch: Partial<SuperpageBlock>) => void;
  onGeometry: (patch: Partial<SuperpageBlock>) => void;
  onRemove: () => void;
  onOpen: () => void;
  onAi: () => void;
  actions: PartActions;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const kindLabel = PART_LABELS[block.kind as PartKind] ?? block.kind;

  const beginDrag = (e: React.PointerEvent) => {
    if (!editing) return;
    if ((e.target as HTMLElement).closest('button,select,input,textarea,a,[data-no-drag]')) return;
    const el = ref.current;
    if (!el) return;
    e.preventDefault();
    // Capture so pointer events keep flowing even when the cursor passes over a
    // child iframe (web-page / HTML parts) during the drag.
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    const sx = e.clientX;
    const sy = e.clientY;
    const ox = block.x ?? 0;
    const oy = block.y ?? 0;
    const move = (ev: PointerEvent) => {
      el.style.left = `${Math.max(0, snap(ox + ev.clientX - sx))}px`;
      el.style.top = `${Math.max(0, snap(oy + ev.clientY - sy))}px`;
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      onGeometry({
        x: Math.max(0, snap(ox + ev.clientX - sx)),
        y: Math.max(0, snap(oy + ev.clientY - sy)),
      });
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };

  const beginResize = (e: React.PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const el = ref.current;
    if (!el) return;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    const sx = e.clientX;
    const sy = e.clientY;
    const ow = block.w ?? 320;
    const oh = block.h ?? 240;
    const move = (ev: PointerEvent) => {
      el.style.width = `${Math.max(MIN_W, snap(ow + ev.clientX - sx))}px`;
      el.style.height = `${Math.max(MIN_H, snap(oh + ev.clientY - sy))}px`;
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      onGeometry({
        w: Math.max(MIN_W, snap(ow + ev.clientX - sx)),
        h: Math.max(MIN_H, snap(oh + ev.clientY - sy)),
      });
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };

  return (
    <div
      ref={ref}
      data-block-id={block.id}
      className="absolute flex flex-col overflow-hidden rounded-lg border border-border bg-card shadow-sm"
      style={{ left: block.x ?? 0, top: block.y ?? 0, width: block.w ?? 320, height: block.h ?? 240 }}
    >
      {editing && (
        <div
          onPointerDown={beginDrag}
          className="flex cursor-move touch-none items-center gap-1 border-b border-border/60 px-1.5 py-1"
        >
          <GripVertical className="h-3.5 w-3.5 text-muted-foreground" aria-hidden />
          <span className="text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground">
            {kindLabel}
          </span>
          <div className="flex-1" />
          <IconBtn label="Ask the assistant about this part" onClick={onAi}>
            <Sparkles className="h-3.5 w-3.5" />
          </IconBtn>
          {openable && (
            <IconBtn label="Open in its editor" onClick={onOpen}>
              <ExternalLink className="h-3.5 w-3.5" />
            </IconBtn>
          )}
          <IconBtn label="Remove part" onClick={onRemove}>
            <Trash2 className="h-3.5 w-3.5" />
          </IconBtn>
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto p-3">
        <PartContent
          block={block}
          payload={payload}
          editing={editing}
          onUpdate={onUpdate}
          actions={actions}
        />
      </div>

      {editing && (
        <button
          type="button"
          aria-label="Resize part"
          onPointerDown={beginResize}
          className="absolute bottom-0 right-0 h-4 w-4 cursor-se-resize touch-none text-muted-foreground"
        >
          <svg viewBox="0 0 10 10" className="h-full w-full" aria-hidden>
            <path d="M9 1v8H1" fill="none" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
          </svg>
        </button>
      )}
    </div>
  );
}

function IconBtn({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
    >
      {children}
    </button>
  );
}
