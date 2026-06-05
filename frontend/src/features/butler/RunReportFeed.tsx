import { Link } from 'react-router-dom';
import { ArrowUpRight, Ban, CheckCircle2, ClipboardList, Loader2, XCircle } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { useRuns } from '@/features/cli/api';
import { useReviewQueue } from '@/features/projects/api';
import type { CliRunSummary } from '@/features/cli/types';
import { useRunReport } from './api';
import { artifactIcon, artifactPath, linkableArtifacts, relativeTime, reportBadges } from './report';

const FEED_LIMIT = 8;
const ARTIFACT_LINK_LIMIT = 6;

/**
 * The butler report: recent runs as cards, each summarizing what the run created
 * (from its provenance-correlated report) with deep links into the knowledge
 * base, plus a banner when agent drafts await review.
 */
export function RunReportFeed() {
  const { data: runs = [] } = useRuns();
  const { data: review } = useReviewQueue();
  const draftCount = (review?.ideas.length ?? 0) + (review?.documents.length ?? 0);

  if (runs.length === 0 && draftCount === 0) return null;

  return (
    <section aria-labelledby="butler-report-heading" className="flex flex-col gap-3">
      <h2
        id="butler-report-heading"
        className="text-sm font-semibold uppercase tracking-wide text-muted-foreground"
      >
        Butler report
      </h2>

      {draftCount > 0 && (
        <Link
          to="/projects"
          className="flex items-center gap-2 rounded-md border border-primary/30 bg-primary/10 px-3 py-2 text-sm transition-colors hover:bg-primary/20"
        >
          <ClipboardList className="h-4 w-4 shrink-0 text-primary-strong" aria-hidden="true" />
          <span>
            {draftCount} draft{draftCount === 1 ? '' : 's'} awaiting your review
          </span>
          <ArrowUpRight className="ml-auto h-4 w-4 shrink-0" aria-hidden="true" />
        </Link>
      )}

      <ul className="flex flex-col gap-3">
        {runs.slice(0, FEED_LIMIT).map((run) => (
          <li key={run.id}>
            <ReportCard run={run} />
          </li>
        ))}
      </ul>
    </section>
  );
}

function StatusIcon({ status }: { status: CliRunSummary['status'] }) {
  switch (status) {
    case 'running':
      return <Loader2 className="h-4 w-4 animate-spin text-primary" aria-label="Running" />;
    case 'succeeded':
      return <CheckCircle2 className="h-4 w-4 text-success" aria-label="Succeeded" />;
    case 'failed':
      return <XCircle className="h-4 w-4 text-danger" aria-label="Failed" />;
    case 'cancelled':
      return <Ban className="h-4 w-4 text-muted-foreground" aria-label="Cancelled" />;
    default:
      return null;
  }
}

function ReportCard({ run }: { run: CliRunSummary }) {
  const finished = run.status !== 'running';
  const { data: report } = useRunReport(run.id, finished);

  const badges = report ? reportBadges(report.counts) : [];
  const links = report ? linkableArtifacts(report.artifacts) : [];

  return (
    <article className="rounded-lg border border-border bg-card p-3">
      <div className="flex items-start gap-2">
        <span className="mt-0.5 shrink-0">
          <StatusIcon status={run.status} />
        </span>
        <p className="min-w-0 flex-1 text-sm line-clamp-2" title={run.prompt}>
          {run.prompt}
        </p>
        <span className="shrink-0 text-xs text-muted-foreground">
          {relativeTime(run.created_at)}
        </span>
      </div>

      {!finished ? (
        <p className="mt-2 pl-6 text-xs text-muted-foreground">The butler is working on it…</p>
      ) : report && report.total === 0 ? (
        <p className="mt-2 pl-6 text-xs text-muted-foreground">
          {run.status === 'succeeded'
            ? 'Read-only run — no knowledge-base changes.'
            : 'No knowledge-base changes.'}
        </p>
      ) : (
        report && (
          <div className="mt-2 flex flex-col gap-2 pl-6">
            <div className="flex flex-wrap gap-1.5">
              {badges.map((b) => (
                <Badge key={b.key} variant="muted">
                  <b.icon className="h-3 w-3" aria-hidden="true" />
                  {b.count} {b.label}
                </Badge>
              ))}
            </div>
            {links.length > 0 && (
              <ul className="flex flex-wrap gap-x-4 gap-y-1">
                {links.slice(0, ARTIFACT_LINK_LIMIT).map((a) => {
                  const to = artifactPath(a);
                  const Icon = artifactIcon(a.target_type);
                  const label = (
                    <>
                      <Icon className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                      <span className="min-w-0 truncate">{a.target_title}</span>
                    </>
                  );
                  return (
                    <li key={`${a.target_type}:${a.target_id}`} className="min-w-0 max-w-full">
                      {to ? (
                        <Link
                          to={to}
                          className="flex max-w-60 items-center gap-1 text-xs text-primary-strong hover:underline"
                        >
                          {label}
                        </Link>
                      ) : (
                        <span className="flex max-w-60 items-center gap-1 text-xs text-muted-foreground">
                          {label}
                        </span>
                      )}
                    </li>
                  );
                })}
                {links.length > ARTIFACT_LINK_LIMIT && (
                  <li className="text-xs text-muted-foreground">
                    +{links.length - ARTIFACT_LINK_LIMIT} more
                  </li>
                )}
              </ul>
            )}
          </div>
        )
      )}
    </article>
  );
}
