import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Check, Copy, ExternalLink, Eye, EyeOff, Globe, Lock, Trash2 } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { ConfirmModal } from '@/components/ui/confirm-modal';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { ExportMenu } from '@/features/documents/ExportMenu';
import { RichTextEditor } from '@/features/documents/RichTextEditor';
import { TagInput } from '@/features/ideas/TagInput';
import { EmptyDetail } from '@/features/objects/EmptyDetail';
import {
  absoluteUrl,
  errorMessage,
  useDeletePage,
  useFolders,
  usePage,
  usePublishPage,
  useUnpublishPage,
  useUpdatePage,
} from './api';
import type { Page, Visibility } from './types';

const selectClass =
  'h-9 rounded-md border border-border bg-transparent px-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring';

/**
 * Editor-only page route. The page list lives in the sidebar "Objects" group;
 * this renders the editor for `/pages/:id` (or a hint when none is selected).
 */
export function PagesPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const detail = usePage(id ?? null);

  if (!id) return <EmptyDetail noun="page" />;
  if (detail.isLoading) {
    return (
      <div className="grid h-[calc(100svh-12rem)] place-items-center">
        <Spinner label="Loading page" />
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
  if (!detail.data) return <EmptyDetail noun="page" />;

  return <PageEditor key={detail.data.id} page={detail.data} onDeleted={() => navigate('/pages')} />;
}

function PageEditor({ page, onDeleted }: { page: Page; onDeleted: () => void }) {
  const [title, setTitle] = useState(page.title);
  const [body, setBody] = useState(page.body);
  const [tags, setTags] = useState<string[]>(page.tags ?? []);
  const [preview, setPreview] = useState(false);
  const [copied, setCopied] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const update = useUpdatePage();
  const publish = usePublishPage();
  const unpublish = useUnpublishPage();
  const remove = useDeletePage();
  const { data: folders = [] } = useFolders();
  const save = update.mutate;

  const tagsKey = JSON.stringify(tags);
  const pageTagsKey = JSON.stringify(page.tags ?? []);

  // Debounced autosave of content (title/body/tags); skips the initial render.
  useEffect(() => {
    if (title === page.title && body === page.body && tagsKey === pageTagsKey) return;
    const timer = setTimeout(
      () => save({ id: page.id, patch: { title: title.trim() || 'Untitled', body, tags } }),
      800,
    );
    return () => clearTimeout(timer);
  }, [title, body, tags, tagsKey, page.id, page.title, page.body, pageTagsKey, save]);

  const dirty = title !== page.title || body !== page.body || tagsKey !== pageTagsKey;
  const status = update.isPending ? 'Saving…' : dirty ? 'Unsaved changes' : `Saved · v${page.version}`;

  const isPublished = page.visibility !== 'draft';
  const shareUrl = absoluteUrl(page.public_url);

  const changeVisibility = (next: Visibility) => {
    if (next === page.visibility) return;
    setPreview(false);
    if (next === 'draft') unpublish.mutate(page.id);
    else publish.mutate({ id: page.id, visibility: next });
  };

  const copyLink = async () => {
    if (!shareUrl) return;
    try {
      await navigator.clipboard.writeText(shareUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard unavailable */
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          aria-label="Page title"
          className="flex-1 text-lg font-semibold"
        />
        <span className="text-xs text-muted-foreground" aria-live="polite">
          {status}
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <label className="flex items-center gap-1 text-sm text-muted-foreground">
          <Globe className="h-4 w-4" aria-hidden="true" />
          <select
            className={selectClass}
            value={page.visibility}
            onChange={(e) => changeVisibility(e.target.value as Visibility)}
            aria-label="Page visibility"
            disabled={publish.isPending || unpublish.isPending}
          >
            <option value="draft">Draft (not served)</option>
            <option value="unlisted">Unlisted (link only)</option>
            <option value="public">Public</option>
          </select>
        </label>

        <select
          className={selectClass}
          value={page.folder_id ?? ''}
          onChange={(e) => save({ id: page.id, patch: { folder_id: e.target.value || null } })}
          aria-label="Page folder"
        >
          <option value="">No folder</option>
          {folders.map((f) => (
            <option key={f.id} value={f.id}>
              {f.name}
            </option>
          ))}
        </select>

        {isPublished && (
          <>
            <Button variant="outline" onClick={() => void copyLink()} aria-label="Copy share link">
              {copied ? <Check className="h-4 w-4" aria-hidden="true" /> : <Copy className="h-4 w-4" aria-hidden="true" />}
              {copied ? 'Copied' : 'Copy link'}
            </Button>
            <Button variant="outline" onClick={() => setPreview((v) => !v)}>
              {preview ? <EyeOff className="h-4 w-4" aria-hidden="true" /> : <Eye className="h-4 w-4" aria-hidden="true" />}
              {preview ? 'Hide preview' : 'Preview'}
            </Button>
            <a
              href={shareUrl}
              target="_blank"
              rel="noreferrer"
              aria-label="Open page in new tab"
              className="inline-flex h-10 w-10 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              <ExternalLink className="h-4 w-4" aria-hidden="true" />
            </a>
          </>
        )}

        <ExportMenu content={body} source="html" filename={title} />
        <Button
          variant="outline"
          size="icon"
          aria-label="Delete page"
          onClick={() => setConfirmDelete(true)}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>

      {page.visibility === 'unlisted' && (
        <p className="flex items-center gap-1 text-xs text-muted-foreground">
          <Lock className="h-3.5 w-3.5" aria-hidden="true" />
          Unlisted: anyone with the link can view. The URL is the only access control.
        </p>
      )}

      {preview && isPublished ? (
        // Preview only via the isolated, cross-origin serve route — never by
        // injecting page HTML into the app DOM. Bare `sandbox` = no scripts,
        // no same-origin (matches the serve CSP).
        <iframe
          title="Page preview"
          src={shareUrl}
          sandbox=""
          className="h-[60vh] w-full rounded-lg border border-border bg-white"
        />
      ) : (
        <>
          <TagInput tags={tags} onChange={setTags} />
          <RichTextEditor initialHtml={page.body} onChange={setBody} />
        </>
      )}

      <ConfirmModal
        open={confirmDelete}
        title="Delete page"
        message={`Delete "${page.title}"? This cannot be undone.`}
        confirmLabel="Delete"
        danger
        pending={remove.isPending}
        error={remove.isError ? errorMessage(remove.error) : null}
        onConfirm={() =>
          remove.mutate(page.id, {
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
