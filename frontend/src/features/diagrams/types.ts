/** draw.io diagram API shapes (Phase 14) — mirror the backend DTOs. */

export type Review = 'draft' | 'published';

export interface DiagramSummary {
  id: string;
  title: string;
  /** Sanitized SVG/PNG `data:` URI thumbnail, or empty. */
  preview: string;
  folder_id: string | null;
  project_id: string | null;
  tags: string[];
  review: Review;
  version: number;
  updated_at: string;
}

export interface Diagram extends DiagramSummary {
  /** mxGraph XML body. */
  xml: string;
  published_at: string | null;
  created_at: string;
}

export interface DiagramListFilters {
  q?: string;
  folder?: string;
}
