import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Pencil, Trash2, X } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/cn';
import { ConfirmModal } from '@/components/ui/confirm-modal';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { ExportMenu } from '@/features/documents/ExportMenu';
import { TagInput } from '@/features/ideas/TagInput';
import { EmptyDetail } from '@/features/objects/EmptyDetail';
import { OBJECT_CANVAS, OBJECT_EDITOR_ROOT } from '@/features/objects/editorLayout';
import { DRAWIO_URL, errorMessage, useDeleteDiagram, useDiagram, useUpdateDiagram } from './api';
import type { Diagram } from './types';

const DRAWIO_ORIGIN = (() => {
  try {
    return new URL(DRAWIO_URL).origin;
  } catch {
    return '';
  }
})();

/**
 * Editor-only diagram route. The diagram list lives in the sidebar "Objects"
 * group; this renders the draw.io editor for `/diagrams/:id` (or a hint).
 */
export function DiagramsPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const detail = useDiagram(id ?? null);

  if (!id) return <EmptyDetail noun="diagram" />;
  if (detail.isLoading) {
    return (
      <div className="grid h-[calc(100svh-12rem)] place-items-center">
        <Spinner label="Loading diagram" />
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
  if (!detail.data) return <EmptyDetail noun="diagram" />;

  return (
    <DiagramEditor
      key={detail.data.id}
      diagram={detail.data}
      onDeleted={() => navigate('/diagrams')}
    />
  );
}

interface DrawioMessage {
  event?: string;
  xml?: string;
  data?: string;
}

function DiagramEditor({ diagram, onDeleted }: { diagram: Diagram; onDeleted: () => void }) {
  const [title, setTitle] = useState(diagram.title);
  const [tags, setTags] = useState<string[]>(diagram.tags ?? []);
  const [editing, setEditing] = useState(true);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const canvasRef = useRef<HTMLDivElement | null>(null);
  // Holds the latest XML reported by the editor, so an `export` message (which
  // may not echo the XML) can be paired with it.
  const lastXml = useRef<string>(diagram.xml);

  const update = useUpdateDiagram();
  const remove = useDeleteDiagram();
  const save = update.mutate;

  // Debounced autosave of title/tags (the XML+preview is saved on editor events).
  const tagsKey = JSON.stringify(tags);
  useEffect(() => {
    const dirty = title !== diagram.title || tagsKey !== JSON.stringify(diagram.tags ?? []);
    if (!dirty) return;
    const timer = setTimeout(
      () => save({ id: diagram.id, patch: { title: title.trim() || 'Untitled diagram', tags } }),
      800,
    );
    return () => clearTimeout(timer);
  }, [title, tags, tagsKey, diagram, save]);

  const post = useCallback(
    (msg: unknown) => iframeRef.current?.contentWindow?.postMessage(JSON.stringify(msg), '*'),
    [],
  );

  // draw.io embed protocol (proto=json). Only handle messages from the configured origin.
  const onMessage = useCallback(
    (e: MessageEvent) => {
      if (DRAWIO_ORIGIN && e.origin !== DRAWIO_ORIGIN) return;
      let msg: DrawioMessage;
      try {
        msg = typeof e.data === 'string' ? JSON.parse(e.data) : (e.data as DrawioMessage);
      } catch {
        return;
      }
      switch (msg.event) {
        case 'init':
          post({ action: 'load', xml: lastXml.current, autosave: 1, fit: 1 });
          break;
        case 'load':
          // Refit when draw.io reports the diagram is ready (container may have resized).
          post({ action: 'fit', border: 8, maxScale: 1 });
          break;
        case 'save':
          if (msg.xml) lastXml.current = msg.xml;
          // Ask for an SVG export (embeds the XML) to use as the static preview.
          post({ action: 'export', format: 'xmlsvg' });
          break;
        case 'export':
          if (msg.data) {
            save({
              id: diagram.id,
              patch: { xml: lastXml.current, preview: msg.data },
            });
          }
          break;
        case 'exit':
          setEditing(false);
          break;
        default:
          break;
      }
    },
    [diagram.id, save],
  );

  useEffect(() => {
    if (!editing) return;
    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, [editing, onMessage]);

  // Tell draw.io to refit when the canvas host is resized (fullscreen, agent dock, etc.).
  useEffect(() => {
    if (!editing || !canvasRef.current || typeof ResizeObserver === 'undefined') return;
    const el = canvasRef.current;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ro = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => post({ action: 'fit', border: 8, maxScale: 1 }), 150);
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
      clearTimeout(timer);
    };
  }, [editing, post]);

  // Full editor chrome (shape library + toolbar). `ui=min` is intentionally omitted.
  const editorSrc = `${DRAWIO_URL}/?embed=1&proto=json&ui=kennedy&libraries=1&spin=1&modified=unsavedChanges&noExitBtn=1`;
  const status = update.isPending ? 'Saving…' : `Saved · v${diagram.version}`;

  return (
    <div className={OBJECT_EDITOR_ROOT}>
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          aria-label="Diagram title"
          className="flex-1 text-lg font-semibold"
        />
        <span className="text-xs text-muted-foreground" aria-live="polite">
          {status}
        </span>
        <Button variant={editing ? 'outline' : 'primary'} onClick={() => setEditing((v) => !v)}>
          {editing ? (
            <>
              <X className="h-4 w-4" aria-hidden="true" />
              Preview
            </>
          ) : (
            <>
              <Pencil className="h-4 w-4" aria-hidden="true" />
              Edit
            </>
          )}
        </Button>
        {/* Export the rendered preview (when present) through the shared pathway. */}
        <ExportMenu
          content={diagram.preview ? `<img src="${diagram.preview}" alt="${title}" />` : `<p>${title}</p>`}
          source="html"
          filename={title}
        />
        <Button
          variant="outline"
          size="icon"
          aria-label="Delete diagram"
          onClick={() => setConfirmDelete(true)}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>

      <div className="shrink-0">
        <TagInput tags={tags} onChange={setTags} />
      </div>

      <div ref={canvasRef} className={cn(OBJECT_CANVAS, 'bg-white')} data-testid="diagram-canvas">
        {editing ? (
          // The live draw.io editor frame — loaded ONLY here, from DRAWIO_URL.
          <iframe
            ref={iframeRef}
            title="draw.io editor"
            src={editorSrc}
            className="absolute inset-0 size-full border-0"
            referrerPolicy="no-referrer"
          />
        ) : diagram.preview ? (
          <div className="absolute inset-0 grid place-items-center p-4">
            <img src={diagram.preview} alt={title} className="max-h-full max-w-full object-contain" />
          </div>
        ) : (
          <div className="absolute inset-0 grid place-items-center text-sm text-muted-foreground">
            No preview yet — click Edit to draw.
          </div>
        )}
      </div>

      <ConfirmModal
        open={confirmDelete}
        title="Delete diagram"
        message={`Delete "${diagram.title}"? This cannot be undone.`}
        confirmLabel="Delete"
        danger
        pending={remove.isPending}
        error={remove.isError ? errorMessage(remove.error) : null}
        onConfirm={() =>
          remove.mutate(diagram.id, {
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
