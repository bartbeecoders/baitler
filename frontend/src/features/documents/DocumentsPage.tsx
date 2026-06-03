import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Trash2 } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { EmptyDetail } from '@/features/objects/EmptyDetail';
import { errorMessage, useDeleteDocument, useDocument, useUpdateDocument } from './api';
import { ExportMenu } from './ExportMenu';
import { RichTextEditor } from './RichTextEditor';
import { TagInput } from '@/features/ideas/TagInput';
import type { Document } from './types';

/**
 * Editor-only document route. The document list now lives in the sidebar
 * "Objects" group; this renders the editor for `/editor/:id` (or a hint).
 */
export function DocumentsPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const detail = useDocument(id ?? null);

  if (!id) return <EmptyDetail noun="document" />;
  if (detail.isLoading) {
    return (
      <div className="grid h-[calc(100svh-12rem)] place-items-center">
        <Spinner label="Loading document" />
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
  if (!detail.data) return <EmptyDetail noun="document" />;

  return (
    <DocumentEditor key={detail.data.id} doc={detail.data} onDeleted={() => navigate('/editor')} />
  );
}

function DocumentEditor({ doc, onDeleted }: { doc: Document; onDeleted: () => void }) {
  const [title, setTitle] = useState(doc.title);
  const [body, setBody] = useState(doc.body);
  const [tags, setTags] = useState<string[]>(doc.tags ?? []);
  const update = useUpdateDocument();
  const remove = useDeleteDocument();
  const save = update.mutate;

  const tagsKey = JSON.stringify(tags);
  const docTagsKey = JSON.stringify(doc.tags ?? []);

  // Debounced autosave; skips the initial (unchanged) render.
  useEffect(() => {
    if (title === doc.title && body === doc.body && tagsKey === docTagsKey) return;
    const timer = setTimeout(
      () =>
        save({ id: doc.id, patch: { title: title.trim() || 'Untitled', body, tags } }),
      800,
    );
    return () => clearTimeout(timer);
  }, [title, body, tags, tagsKey, doc.id, doc.title, doc.body, docTagsKey, save]);

  const dirty = title !== doc.title || body !== doc.body || tagsKey !== docTagsKey;
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
      <TagInput tags={tags} onChange={setTags} />
      <RichTextEditor initialHtml={doc.body} onChange={setBody} />
    </div>
  );
}
