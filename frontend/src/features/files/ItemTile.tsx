import {
  Download,
  File,
  FileText,
  Folder,
  Image,
  Music,
  Pencil,
  Trash2,
  Video,
} from 'lucide-react';

import type { FileItem, FolderItem } from './types';
import { formatBytes } from './util';

interface FolderTileProps {
  kind: 'folder';
  item: FolderItem;
}
interface FileTileProps {
  kind: 'file';
  item: FileItem;
}
type Props = (FolderTileProps | FileTileProps) & {
  onOpen: () => void;
  onRename: () => void;
  onDelete: () => void;
  onDownload?: () => void;
};

/** Module-scope glyph component (keeps the icon choice out of render-time assignment). */
function FileGlyph({ mime, className }: { mime: string; className?: string }) {
  if (mime.startsWith('image/')) return <Image className={className} aria-hidden="true" />;
  if (mime.startsWith('video/')) return <Video className={className} aria-hidden="true" />;
  if (mime.startsWith('audio/')) return <Music className={className} aria-hidden="true" />;
  if (mime.startsWith('text/') || mime === 'application/json' || mime === 'application/pdf') {
    return <FileText className={className} aria-hidden="true" />;
  }
  return <File className={className} aria-hidden="true" />;
}

function ActionButton({
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
      aria-label={label}
      title={label}
      onClick={onClick}
      className="rounded-md bg-card/80 p-1.5 text-muted-foreground shadow-sm hover:bg-muted hover:text-foreground"
    >
      {children}
    </button>
  );
}

/** A single folder or file in the grid, with hover/focus-revealed actions. */
export function ItemTile(props: Props) {
  const { onOpen, onRename, onDelete, onDownload } = props;
  const item = props.item;
  const subtitle = props.kind === 'file' ? formatBytes(props.item.size) : 'Folder';

  return (
    <div className="group relative">
      <button
        type="button"
        onClick={onOpen}
        aria-label={props.kind === 'folder' ? `Open folder ${item.name}` : `Preview ${item.name}`}
        className="flex w-full flex-col items-start gap-3 rounded-lg border border-border bg-card p-4 text-left transition-colors hover:border-primary/50"
      >
        {props.kind === 'folder' ? (
          <Folder className="h-8 w-8 text-primary-strong" aria-hidden="true" />
        ) : (
          <FileGlyph mime={props.item.mime} className="h-8 w-8 text-muted-foreground" />
        )}
        <div className="w-full">
          <p className="truncate text-sm font-medium" title={item.name}>
            {item.name}
          </p>
          <p className="text-xs text-muted-foreground">{subtitle}</p>
        </div>
      </button>

      <div className="absolute right-2 top-2 flex gap-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
        {props.kind === 'file' && onDownload && (
          <ActionButton label={`Download ${item.name}`} onClick={onDownload}>
            <Download className="h-4 w-4" />
          </ActionButton>
        )}
        <ActionButton label={`Rename ${item.name}`} onClick={onRename}>
          <Pencil className="h-4 w-4" />
        </ActionButton>
        <ActionButton label={`Delete ${item.name}`} onClick={onDelete}>
          <Trash2 className="h-4 w-4" />
        </ActionButton>
      </div>
    </div>
  );
}
