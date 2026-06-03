/** Mindmap API shapes (Phase 14) — mirror the backend DTOs. */

export type Review = 'draft' | 'published';
export type MindmapSourceFormat = 'json' | 'markdown';

export interface MindmapNode {
  id: string;
  label: string;
  parent?: string | null;
  x?: number | null;
  y?: number | null;
  color?: string | null;
  item_type?: string | null;
  item_id?: string | null;
}

export interface MindmapEdge {
  from: string;
  to: string;
  label?: string | null;
}

export interface MindmapGraph {
  nodes: MindmapNode[];
  edges: MindmapEdge[];
}

export interface MindmapSummary {
  id: string;
  title: string;
  source_format: MindmapSourceFormat;
  folder_id: string | null;
  project_id: string | null;
  tags: string[];
  review: Review;
  version: number;
  updated_at: string;
}

export interface Mindmap extends MindmapSummary {
  graph: MindmapGraph;
  published_at: string | null;
  created_at: string;
}

export interface MindmapListFilters {
  q?: string;
  folder?: string;
}
