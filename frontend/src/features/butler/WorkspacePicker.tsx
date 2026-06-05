import { useState } from 'react';
import { CornerLeftUp, Folder, FolderOpen } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Modal } from '@/components/ui/modal';
import { Spinner } from '@/components/ui/spinner';
import { useWorkspaceBrowse } from './api';

export interface WorkspacePickerProps {
  open: boolean;
  onClose: () => void;
  /** Called with the chosen canonical folder path. */
  onPick: (path: string) => void;
}

/**
 * Server-side folder picker over the allow-listed `CLAUDE_CLI_WORKSPACE_ROOTS`:
 * the top level shows the roots, clicking descends, and "Use this folder" grants
 * the current directory to the run. Only directories the server may grant are
 * ever shown (the browse endpoint re-validates every path).
 */
export function WorkspacePicker({ open, onClose, onPick }: WorkspacePickerProps) {
  const [path, setPath] = useState<string | null>(null);
  const { data, isLoading, error } = useWorkspaceBrowse(path, open);

  const pick = () => {
    if (data?.path) {
      onPick(data.path);
      onClose();
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="Point the butler at a folder">
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2 text-sm">
          <FolderOpen className="h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
          <span className="min-w-0 truncate font-mono text-xs" title={data?.path ?? undefined}>
            {data?.path ?? 'Allowed folders'}
          </span>
          {data?.path && (
            <Button
              variant="ghost"
              size="sm"
              className="ml-auto shrink-0"
              onClick={() => setPath(data.parent)}
              aria-label="Up one folder"
            >
              <CornerLeftUp className="h-4 w-4" aria-hidden="true" />
              Up
            </Button>
          )}
        </div>

        <div className="max-h-72 overflow-y-auto rounded-md border border-border">
          {isLoading ? (
            <div className="grid place-items-center py-8">
              <Spinner label="Reading folders" />
            </div>
          ) : error ? (
            <p className="px-3 py-4 text-sm text-danger" role="alert">
              Could not read this folder.
            </p>
          ) : data && data.dirs.length === 0 ? (
            <p className="px-3 py-4 text-sm text-muted-foreground">
              {data.roots.length === 0
                ? 'No folders are allow-listed. Set CLAUDE_CLI_WORKSPACE_ROOTS on the server.'
                : 'No subfolders here.'}
            </p>
          ) : (
            <ul>
              {data?.dirs.map((d) => (
                <li key={d.path}>
                  <button
                    type="button"
                    onClick={() => setPath(d.path)}
                    className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors hover:bg-muted"
                  >
                    <Folder className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                    <span className="min-w-0 truncate">{d.name}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
          {data?.truncated && (
            <p className="border-t border-border px-3 py-2 text-xs text-muted-foreground">
              Showing the first {data.dirs.length} folders.
            </p>
          )}
        </div>

        <div className="flex items-center justify-between gap-2">
          <p className="text-xs text-muted-foreground">
            The butler gets read-only access to the chosen folder for this conversation.
          </p>
          <Button onClick={pick} disabled={!data?.path}>
            Use this folder
          </Button>
        </div>
      </div>
    </Modal>
  );
}
