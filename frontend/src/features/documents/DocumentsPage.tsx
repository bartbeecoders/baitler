import { useEffect, useRef, useState, type ChangeEvent } from 'react';
import { FileText, Plus, Trash2, Upload } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import {
  errorMessage,
  markdownToHtml,
  useCreateDocument,
  useDeleteDocument,
  useDocument,
  useDocuments,
  useUpdateDocument,
} from './api';
import { ExportMenu } from './ExportMenu';
import { RichTextEditor } from './RichTextEditor';
import type { Document } from './types';

export function DocumentsPage() {
  const { data: docs = [], isLoading } = useDocuments();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const create = useCreateDocument();
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Default to the most recent document (derived, so no state sync needed).
  const effectiveId = selectedId ?? docs[0]?.id ?? null;
  const detail = useDocument(effectiveId);

  const newDoc = () =>
    create.mutate(
      { title: 'Untitled', body: '<p></p>' },
      { onSuccess: (doc) => setSelectedId(doc.id) },
    );

  const importMarkdown = async (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = '';
    if (!file) return;
    const text = await file.text();
    const html = await markdownToHtml(text);
    const title = file.name.replace(/\.md$/i, '') || 'Imported';
    create.mutate({ title, body: html }, { onSuccess: (doc) => setSelectedId(doc.id) });
  };

  return (
    <div className="flex h-[calc(100svh-9rem)] gap-4">
      <aside className="flex w-64 shrink-0 flex-col gap-2">
        <div className="flex gap-2">
          <Button className="flex-1" onClick={newDoc} disabled={create.isPending}>
            <Plus className="h-4 w-4" aria-hidden="true" />
            New
          </Button>
          <Button variant="outline" size="icon" aria-label="Import Markdown" onClick={() => fileInputRef.current?.click()}>
            <Upload className="h-4 w-4" />
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept=".md,text/markdown"
            className="hidden"
            onChange={(e) => void importMarkdown(e)}
          />
        </div>
        <div className="flex-1 overflow-auto rounded-lg border border-border">
          {isLoading ? (
            <div className="grid place-items-center py-10">
              <Spinner label="Loading documents" />
            </div>
          ) : docs.length === 0 ? (
            <p className="p-4 text-sm text-muted-foreground">No documents yet.</p>
          ) : (
            <ul>
              {docs.map((doc) => (
                <li key={doc.id}>
                  <button
                    type="button"
                    onClick={() => setSelectedId(doc.id)}
                    className={cn(
                      'flex w-full items-center gap-2 px-3 py-2 text-left text-sm',
                      doc.id === effectiveId ? 'bg-accent text-accent-foreground' : 'hover:bg-muted',
                    )}
                  >
                    <FileText className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                    <span className="truncate">{doc.title}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </aside>

      <section className="flex-1 overflow-auto">
        {!effectiveId ? (
          <div className="grid h-full place-items-center text-sm text-muted-foreground">
            Select a document, or create a new one.
          </div>
        ) : detail.isLoading ? (
          <div className="grid h-full place-items-center">
            <Spinner label="Loading document" />
          </div>
        ) : detail.isError ? (
          <p className="text-sm text-danger" role="alert">
            {errorMessage(detail.error)}
          </p>
        ) : detail.data ? (
          <DocumentEditor key={detail.data.id} doc={detail.data} onDeleted={() => setSelectedId(null)} />
        ) : null}
      </section>
    </div>
  );
}

function DocumentEditor({ doc, onDeleted }: { doc: Document; onDeleted: () => void }) {
  const [title, setTitle] = useState(doc.title);
  const [body, setBody] = useState(doc.body);
  const update = useUpdateDocument();
  const remove = useDeleteDocument();
  const save = update.mutate;

  // Debounced autosave; skips the initial (unchanged) render.
  useEffect(() => {
    if (title === doc.title && body === doc.body) return;
    const timer = setTimeout(
      () => save({ id: doc.id, patch: { title: title.trim() || 'Untitled', body } }),
      800,
    );
    return () => clearTimeout(timer);
  }, [title, body, doc.id, doc.title, doc.body, save]);

  const dirty = title !== doc.title || body !== doc.body;
  const status = update.isPending ? 'Saving…' : dirty ? 'Unsaved changes' : `Saved · v${doc.version}`;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          aria-label="Document title"
          className="flex-1 text-lg font-semibold"
        />
        <span className="text-xs text-muted-foreground" aria-live="polite">
          {status}
        </span>
        <ExportMenu content={body} source="html" filename={title} />
        <Button
          variant="outline"
          size="icon"
          aria-label="Delete document"
          onClick={() => {
            if (window.confirm(`Delete "${doc.title}"?`)) remove.mutate(doc.id, { onSuccess: onDeleted });
          }}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
      <RichTextEditor initialHtml={doc.body} onChange={setBody} />
    </div>
  );
}
