/** Hosted-page API shapes (Phase 12) — mirror the backend DTOs. */

export type Visibility = 'draft' | 'unlisted' | 'public';
export type SourceFormat = 'html' | 'markdown';

export interface PageSummary {
  id: string;
  title: string;
  slug: string;
  visibility: Visibility;
  source_format: SourceFormat;
  folder_id: string | null;
  project_id: string | null;
  version: number;
  published_at: string | null;
  tags: string[];
  /** Absolute when `PUBLIC_PAGE_ORIGIN` is set, else origin-relative (`/p/{slug}`); empty for drafts. */
  public_url: string;
  updated_at: string;
}

export interface Page extends PageSummary {
  /** Sanitized HTML body. */
  body: string;
  created_at: string;
}

export interface PageListFilters {
  q?: string;
  visibility?: Visibility;
  folder?: string;
}

export const VISIBILITIES: Visibility[] = ['draft', 'unlisted', 'public'];

/** The visibility states a published page can take (driven by the publish toggle). */
export const PUBLISH_VISIBILITIES: Exclude<Visibility, 'draft'>[] = ['unlisted', 'public'];
