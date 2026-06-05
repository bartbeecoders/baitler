import {
  FileText,
  Folder,
  FolderKanban,
  FolderOpen,
  Globe,
  LayoutGrid,
  Lightbulb,
  Link2,
  Network,
  Pencil,
  PenTool,
  Trash2,
  type LucideIcon,
} from 'lucide-react';

import type { ReportArtifact } from './api';

/** One rendered count badge on a report card. */
export interface ReportBadge {
  key: string;
  label: string;
  icon: LucideIcon;
  count: number;
}

/** Sum the counts whose action keys satisfy `pred`. */
function sum(counts: Record<string, number>, pred: (action: string) => boolean): number {
  return Object.entries(counts).reduce((acc, [k, v]) => (pred(k) ? acc + v : acc), 0);
}

/**
 * Fold a report's per-action counts (`idea.create: 4, file.import: 1, …`) into
 * the badge strip: created artifacts per type, plus rolled-up update/link/delete
 * counts. Order is display order.
 */
export function reportBadges(counts: Record<string, number>): ReportBadge[] {
  const one = (k: string) => counts[k] ?? 0;
  const badges: ReportBadge[] = [
    { key: 'ideas', label: 'ideas', icon: Lightbulb, count: one('idea.create') },
    { key: 'documents', label: 'documents', icon: FileText, count: one('document.create') },
    { key: 'pages', label: 'pages', icon: Globe, count: one('page.create') },
    {
      key: 'files',
      label: 'files',
      icon: FolderOpen,
      count: one('file.create') + one('file.import'),
    },
    { key: 'folders', label: 'folders', icon: Folder, count: one('folder.create') },
    { key: 'mindmaps', label: 'mindmaps', icon: Network, count: one('mindmap.create') },
    { key: 'diagrams', label: 'diagrams', icon: PenTool, count: one('diagram.create') },
    { key: 'superpages', label: 'superpages', icon: LayoutGrid, count: one('superpage.create') },
    { key: 'projects', label: 'projects', icon: FolderKanban, count: one('project.create') },
    { key: 'links', label: 'links', icon: Link2, count: one('knowledge.link') },
    {
      key: 'updated',
      label: 'updated',
      icon: Pencil,
      count: sum(counts, (a) => a.endsWith('.update') || a.endsWith('.publish')),
    },
    { key: 'deleted', label: 'deleted', icon: Trash2, count: sum(counts, (a) => a.endsWith('.delete')) },
  ];
  return badges.filter((b) => b.count > 0);
}

const TYPE_ICONS: Record<string, LucideIcon> = {
  idea: Lightbulb,
  document: FileText,
  page: Globe,
  file: FolderOpen,
  folder: Folder,
  mindmap: Network,
  diagram: PenTool,
  superpage: LayoutGrid,
  project: FolderKanban,
  link: Link2,
};

export function artifactIcon(targetType: string): LucideIcon {
  return TYPE_ICONS[targetType] ?? FileText;
}

/** Deep-link for an artifact, `null` when there is no sensible detail route. */
export function artifactPath(a: ReportArtifact): string | null {
  if (!a.target_id) return null;
  switch (a.target_type) {
    case 'idea':
      return `/ideas/${a.target_id}`;
    case 'document':
      return `/editor/${a.target_id}`;
    case 'page':
      return `/pages/${a.target_id}`;
    case 'mindmap':
      return `/mindmaps/${a.target_id}`;
    case 'diagram':
      return `/diagrams/${a.target_id}`;
    case 'superpage':
      return `/superpages/${a.target_id}`;
    case 'file':
    case 'folder':
      return '/files';
    case 'project':
      return '/projects';
    default:
      return null;
  }
}

/**
 * The artifacts worth linking on a card: surviving targets (skip deletes/unlinks),
 * deduped by type+id, newest first (the input is already newest-first).
 */
export function linkableArtifacts(artifacts: ReportArtifact[]): ReportArtifact[] {
  const seen = new Set<string>();
  const out: ReportArtifact[] = [];
  for (const a of artifacts) {
    if (a.action.endsWith('.delete') || a.action.endsWith('.unlink')) continue;
    if (!a.target_id || !a.target_title) continue;
    const key = `${a.target_type}:${a.target_id}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(a);
  }
  return out;
}

/** Compact "2h ago"-style timestamp for the feed. */
export function relativeTime(iso: string, now: Date = new Date()): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return '';
  const s = Math.max(0, Math.floor((now.getTime() - then) / 1000));
  if (s < 60) return 'just now';
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}d ago`;
  return new Date(iso).toLocaleDateString();
}
