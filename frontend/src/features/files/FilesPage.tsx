import { useEffect, useRef, useState, type DragEvent } from 'react';
import { FolderPlus, Search, Upload } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { ConfirmModal } from '@/components/ui/confirm-modal';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import { Breadcrumbs } from './Breadcrumbs';
import { ItemTile } from './ItemTile';
import { NewFolderModal } from './NewFolderModal';
import { PreviewModal } from './PreviewModal';
import { RenameModal } from './RenameModal';
import {
  errorMessage,
  useDeleteItem,
  useDownload,
  useListing,
  useUpload,
  type ItemKind,
} from './api';
import type { FileItem } from './types';

/** Debounce a rapidly-changing value (e.g. a search box). */
function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return debounced;
}

type RenameTarget = { kind: ItemKind; id: string; name: string };
type DeleteTarget = { kind: ItemKind; id: string; name: string };

export function FilesPage() {
  const [folderId, setFolderId] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const debouncedQuery = useDebounced(query.trim(), 250);
  const searching = debouncedQuery.length > 0;

  const listing = useListing(folderId, debouncedQuery);
  const upload = useUpload();
  const remove = useDeleteItem();
  const download = useDownload();

  const [newFolderOpen, setNewFolderOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<RenameTarget | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null);
  const [previewFile, setPreviewFile] = useState<FileItem | null>(null);
  const [dragging, setDragging] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const navigateTo = (id: string | null) => {
    // Clear any lingering mutation error so a stale banner doesn't follow us.
    upload.reset();
    remove.reset();
    download.reset();
    setQuery('');
    setFolderId(id);
  };

  const doUpload = (files: FileList | File[]) => {
    const list = Array.from(files);
    if (list.length > 0) upload.mutate({ files: list, folderId });
  };

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragging(false);
    doUpload(e.dataTransfer.files);
  };

  const confirmDelete = () => {
    if (!deleteTarget) return;
    remove.mutate(
      { kind: deleteTarget.kind, id: deleteTarget.id },
      { onSuccess: () => setDeleteTarget(null) },
    );
  };

  const data = listing.data;
  const isEmpty = data && data.folders.length === 0 && data.files.length === 0;
  const actionError = upload.error ?? download.error;

  // Derived, announced when it changes (no setState-in-effect needed).
  const liveStatus = listing.isLoading
    ? 'Loading files'
    : listing.isError
      ? 'Failed to load files'
      : searching
        ? `${data?.files.length ?? 0} matching files`
        : data
          ? `${data.folders.length} folders, ${data.files.length} files`
          : '';

  return (
    <div className="flex flex-col gap-6">
      <p className="sr-only" role="status" aria-live="polite">
        {liveStatus}
      </p>

      <div>
        <h1 className="text-2xl font-bold tracking-tight">Files</h1>
        <p className="text-sm text-muted-foreground">Store, organize, and preview your files.</p>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3">
        {searching ? (
          <p className="text-sm text-muted-foreground">Search results for “{debouncedQuery}”</p>
        ) : (
          <Breadcrumbs breadcrumbs={data?.breadcrumbs ?? []} onNavigate={navigateTo} />
        )}

        <div className="flex items-center gap-2">
          <div className="relative">
            <Search
              className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <Input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search files…"
              aria-label="Search files"
              className="w-44 pl-8"
            />
          </div>
          <Button variant="outline" onClick={() => setNewFolderOpen(true)} disabled={searching}>
            <FolderPlus className="h-4 w-4" aria-hidden="true" />
            New folder
          </Button>
          <Button
            onClick={() => fileInputRef.current?.click()}
            disabled={searching || upload.isPending}
          >
            {upload.isPending ? (
              <Spinner className="h-4 w-4" label="Uploading" />
            ) : (
              <Upload className="h-4 w-4" aria-hidden="true" />
            )}
            Upload
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            multiple
            className="hidden"
            onChange={(e) => {
              if (e.target.files) doUpload(e.target.files);
              e.target.value = '';
            }}
          />
        </div>
      </div>

      {actionError && (
        <p
          className="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger"
          role="alert"
        >
          {errorMessage(actionError)}
        </p>
      )}

      <div
        onDragOver={(e) => {
          e.preventDefault();
          if (!searching) setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={searching ? undefined : onDrop}
        aria-busy={listing.isFetching}
        className={cn(
          'min-h-64 rounded-lg border-2 border-dashed p-4 transition-colors',
          dragging ? 'border-primary bg-accent/40' : 'border-transparent',
        )}
      >
        {listing.isLoading ? (
          <div className="grid place-items-center py-16 text-muted-foreground">
            <Spinner label="Loading files" />
          </div>
        ) : listing.isError ? (
          <p className="py-16 text-center text-sm text-danger" role="alert">
            {errorMessage(listing.error)}
          </p>
        ) : isEmpty ? (
          <div className="grid place-items-center gap-1 py-16 text-center">
            <p className="font-medium">
              {searching ? 'No matching files' : 'This folder is empty'}
            </p>
            {!searching && (
              <p className="text-sm text-muted-foreground">
                Drag files here, or use Upload / New folder.
              </p>
            )}
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {data?.folders.map((folder) => (
              <ItemTile
                key={folder.id}
                kind="folder"
                item={folder}
                onOpen={() => navigateTo(folder.id)}
                onRename={() => setRenameTarget({ kind: 'folder', id: folder.id, name: folder.name })}
                onDelete={() => setDeleteTarget({ kind: 'folder', id: folder.id, name: folder.name })}
              />
            ))}
            {data?.files.map((file) => (
              <ItemTile
                key={file.id}
                kind="file"
                item={file}
                onOpen={() => setPreviewFile(file)}
                onDownload={() => download.mutate({ id: file.id, name: file.name })}
                onRename={() => setRenameTarget({ kind: 'file', id: file.id, name: file.name })}
                onDelete={() => setDeleteTarget({ kind: 'file', id: file.id, name: file.name })}
              />
            ))}
          </div>
        )}
      </div>

      <NewFolderModal
        open={newFolderOpen}
        onClose={() => setNewFolderOpen(false)}
        parentId={folderId}
      />
      {renameTarget && (
        <RenameModal
          key={renameTarget.id}
          open
          onClose={() => setRenameTarget(null)}
          kind={renameTarget.kind}
          id={renameTarget.id}
          currentName={renameTarget.name}
        />
      )}
      <ConfirmModal
        open={!!deleteTarget}
        title={`Delete ${deleteTarget?.kind ?? 'item'}`}
        message={`Delete “${deleteTarget?.name ?? ''}”? This cannot be undone.`}
        confirmLabel="Delete"
        danger
        pending={remove.isPending}
        error={remove.isError ? errorMessage(remove.error) : null}
        onConfirm={confirmDelete}
        onClose={() => {
          remove.reset();
          setDeleteTarget(null);
        }}
      />
      {previewFile && (
        <PreviewModal key={previewFile.id} file={previewFile} onClose={() => setPreviewFile(null)} />
      )}
    </div>
  );
}
