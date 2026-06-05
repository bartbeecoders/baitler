import type { MindmapGraph } from '@/features/mindmaps/types';

export type Review = 'draft' | 'published';

/** The part kinds the canvas authors. */
export type PartKind = 'text' | 'code' | 'image' | 'file' | 'webpage' | 'mindmap' | 'diagram';
/** Plus legacy v1 kinds kept readable for older boards. */
export type BlockKind = PartKind | 'embed' | 'note' | 'heading';

export type ImageSource = 'upload' | 'url' | 'generated';
export type WebKind = 'url' | 'page';
export type EmbedItemType = 'idea' | 'document' | 'file' | 'page' | 'mindmap' | 'diagram';

/** Add-menu order. */
export const PART_KINDS: PartKind[] = [
  'text',
  'image',
  'code',
  'file',
  'webpage',
  'mindmap',
  'diagram',
];

export const PART_LABELS: Record<PartKind, string> = {
  text: 'Text',
  image: 'Image',
  code: 'Code',
  file: 'File',
  webpage: 'Web page',
  mindmap: 'Mindmap',
  diagram: 'Diagram',
};

/** Labels for the item picker, which selects by referenceable item type. */
export const EMBED_TYPE_LABELS: Record<EmbedItemType, string> = {
  idea: 'Idea',
  document: 'Document',
  page: 'Page',
  file: 'File',
  mindmap: 'Mindmap',
  diagram: 'Diagram',
};

export interface SuperpageBlock {
  id: string;
  kind: BlockKind;
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  /** Reference to a Baitler item (file/page/mindmap/diagram, or legacy embed). */
  item_type?: EmbedItemType;
  item_id?: string;
  /** Inline content. */
  markdown?: string;
  /** Code body, image caption, or legacy heading text. */
  text?: string;
  lang?: string;
  url?: string;
  src?: ImageSource;
  web_kind?: WebKind;
}

export interface SuperpageLayout {
  layout: string;
  blocks: SuperpageBlock[];
}

export interface SuperpageSummary {
  id: string;
  title: string;
  folder_id?: string | null;
  project_id?: string | null;
  tags: string[];
  review: Review;
  block_count: number;
  updated_at: string;
}

export interface Superpage {
  id: string;
  title: string;
  blocks: SuperpageLayout;
  folder_id?: string | null;
  project_id?: string | null;
  tags: string[];
  review: Review;
  version: number;
  created_at: string;
  updated_at: string;
}

export interface SuperpageListFilters {
  q?: string;
  folder?: string;
  project?: string;
  tag?: string;
}

/**
 * Resolved preview for one part, from `GET /superpages/{id}/context`. The
 * backend flattens the per-kind payload onto `{id,kind}`.
 */
export interface ResolvedBlockPayload {
  title?: string | null;
  missing?: boolean;
  // text / code / heading
  markdown?: string;
  text?: string;
  lang?: string;
  // image
  src?: ImageSource;
  caption?: string | null;
  // file & image(upload)
  item_id?: string;
  name?: string | null;
  mime?: string | null;
  size?: number | null;
  // webpage
  web_kind?: WebKind;
  url?: string | null;
  format?: 'markdown' | 'html';
  body?: string;
  public_url?: string;
  visibility?: string;
  // mindmap / diagram
  graph?: MindmapGraph;
  has_xml?: boolean;
  preview?: string | null;
  // legacy embed
  item_type?: EmbedItemType;
}

export interface SuperpageContextBlock extends ResolvedBlockPayload {
  id: string;
  kind: BlockKind;
}
