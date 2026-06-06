/** Knowledge-layer (Phase 11) API shapes — mirror the backend DTOs. */

export type ProjectStatus = 'active' | 'archived';
export type Review = 'draft' | 'published';
export type ItemType = 'idea' | 'document' | 'file';

export interface Project {
  id: string;
  name: string;
  slug: string;
  summary: string;
  status: ProjectStatus;
  created_at: string;
  updated_at: string;
}

export interface MemberItem {
  id: string;
  title: string;
  review?: Review;
}

export interface MemberCounts {
  ideas: number;
  documents: number;
  files: number;
  drafts: number;
}

export interface ProjectMembers {
  ideas: MemberItem[];
  documents: MemberItem[];
  files: MemberItem[];
}

export interface ProjectDetail extends Project {
  counts: MemberCounts;
  members: ProjectMembers;
}

export interface ReviewQueue {
  ideas: MemberItem[];
  documents: MemberItem[];
  /** Draft plugins awaiting approval (Phase 16; approve/enable lives at
   * /plugins/{id}/... — full management UI lands with 16.C). */
  plugins?: MemberItem[];
}

export interface Activity {
  id: string;
  agent: string | null;
  action: string;
  target_type: string;
  target_id: string;
  target_title: string;
  project_id: string | null;
  summary: string;
  created_at: string;
}

/** The portal route that owns each item type (for "view in …" links). */
export const FEATURE_ROUTE: Record<ItemType, string> = {
  idea: '/ideas',
  document: '/editor',
  file: '/files',
};
