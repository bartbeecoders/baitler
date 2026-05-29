/** Idea management API shapes (mirror the backend DTOs). */

export type IdeaStatus = 'inbox' | 'active' | 'done' | 'archived';

export const STATUSES: IdeaStatus[] = ['inbox', 'active', 'done', 'archived'];

export const STATUS_LABELS: Record<IdeaStatus, string> = {
  inbox: 'Inbox',
  active: 'Active',
  done: 'Done',
  archived: 'Archived',
};

export interface Idea {
  id: string;
  title: string;
  body: string;
  tags: string[];
  status: IdeaStatus;
  links: string[];
  created_at: string;
  updated_at: string;
}

export interface IdeaSummary {
  id: string;
  title: string;
  status: IdeaStatus;
}

export interface IdeaDetail extends Idea {
  related: IdeaSummary[];
}

export interface IdeaFilters {
  status: IdeaStatus | null;
  tag: string | null;
  q: string;
}
