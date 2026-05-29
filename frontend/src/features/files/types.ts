/** File-manager API shapes (mirror the backend DTOs). */

export interface FileItem {
  id: string;
  name: string;
  mime: string;
  size: number;
  folder_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface FolderItem {
  id: string;
  name: string;
  parent_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface Listing {
  /** The folder being listed (`null` at the root). */
  folder: FolderItem | null;
  /** Path from the root to the current folder. */
  breadcrumbs: FolderItem[];
  folders: FolderItem[];
  files: FileItem[];
}
