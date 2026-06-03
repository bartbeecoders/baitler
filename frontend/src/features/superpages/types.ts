export type Review = 'draft' | 'published';

export type BlockKind = 'embed' | 'note' | 'heading';

export type EmbedItemType = 'idea' | 'document' | 'file' | 'page' | 'mindmap' | 'diagram';

export interface SuperpageBlock {
  id: string;
  kind: BlockKind;
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  item_type?: EmbedItemType;
  item_id?: string;
  markdown?: string;
  text?: string;
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