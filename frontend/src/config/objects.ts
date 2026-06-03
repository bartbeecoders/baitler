import { FileText, Globe, Lightbulb, Network, PenTool, type LucideIcon } from 'lucide-react';

/** A content type listed under the sidebar "Objects" group (Phase 14 layout). */
export interface ObjectType {
  key: string;
  label: string;
  /** Singular noun used in list copy ("New document", "Search documents…"). */
  noun: string;
  icon: LucideIcon;
  /** Route base; detail lives at `${basePath}/${id}`. */
  basePath: string;
}

export const OBJECT_TYPES: ObjectType[] = [
  { key: 'documents', label: 'Documents', noun: 'document', icon: FileText, basePath: '/editor' },
  { key: 'ideas', label: 'Ideas', noun: 'idea', icon: Lightbulb, basePath: '/ideas' },
  { key: 'pages', label: 'Pages', noun: 'page', icon: Globe, basePath: '/pages' },
  { key: 'mindmaps', label: 'Mindmaps', noun: 'mindmap', icon: Network, basePath: '/mindmaps' },
  { key: 'diagrams', label: 'Diagrams', noun: 'diagram', icon: PenTool, basePath: '/diagrams' },
];

/** Route bases owned by object types — used to filter them out of the primary nav. */
export const OBJECT_BASE_PATHS: string[] = OBJECT_TYPES.map((t) => t.basePath);
