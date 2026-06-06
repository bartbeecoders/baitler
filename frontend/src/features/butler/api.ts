import { useQuery } from '@tanstack/react-query';

import { apiFetch } from '@/lib/api';
import { CLI_ROOT } from '@/features/cli/api';
import type { CliRunSummary } from '@/features/cli/types';

/** One provenance row from a run's report (an `activity` entry). */
export interface ReportArtifact {
  id: string;
  agent: string | null;
  run_id: string | null;
  /** e.g. `idea.create`, `file.import`, `page.publish`. */
  action: string;
  target_type: string;
  target_id: string;
  target_title: string;
  project_id: string | null;
  summary: string;
  created_at: string;
}

/** GET /cli/runs/{id}/report — what a run actually did to the knowledge base. */
export interface RunReport {
  run: CliRunSummary;
  artifacts: ReportArtifact[];
  counts: Record<string, number>;
  total: number;
}

export function useRunReport(id: string, enabled = true) {
  return useQuery({
    queryKey: [...CLI_ROOT, 'report', id],
    queryFn: () => apiFetch<RunReport>(`/cli/runs/${id}/report`),
    enabled,
    // A finished run's provenance is immutable (the activity log is append-only
    // and keyed to the run), so never refetch it.
    staleTime: Infinity,
  });
}

export interface BrowseEntry {
  name: string;
  path: string;
}

/** GET /cli/workspace/browse — directories under the allow-listed roots. */
export interface BrowseResponse {
  roots: string[];
  /** Canonical path being listed (`null` ⇒ the roots themselves). */
  path: string | null;
  /** Canonical parent, present only while it is still under an allowed root. */
  parent: string | null;
  dirs: BrowseEntry[];
  truncated: boolean;
}

export function useWorkspaceBrowse(path: string | null, enabled = true) {
  return useQuery({
    queryKey: ['cli', 'workspace', 'browse', path ?? ''],
    queryFn: () =>
      apiFetch<BrowseResponse>(
        path ? `/cli/workspace/browse?path=${encodeURIComponent(path)}` : '/cli/workspace/browse',
      ),
    enabled,
  });
}
