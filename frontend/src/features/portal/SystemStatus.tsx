import { apiStatusOf, useHealth, useVersion } from '@/hooks/useSystemStatus';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { cn } from '@/lib/cn';

/** A status row's health: `null` while loading, else up/down. */
type RowState = { label: string; value: string; ok: boolean | null };

/** Live panel summarizing the backend connection (API, DB, version). */
export function SystemStatus() {
  const health = useHealth();
  const version = useVersion();
  const apiStatus = apiStatusOf(health);

  const rows: RowState[] = [
    {
      label: 'API',
      value: apiStatus,
      ok: apiStatus === 'loading' ? null : apiStatus === 'connected',
    },
    {
      label: 'Database',
      value: health.isLoading ? '…' : (health.data?.db ?? 'unknown'),
      ok: health.isLoading ? null : health.data?.db === 'up',
    },
    {
      label: 'Backend',
      value: version.isLoading
        ? '…'
        : version.data
          ? `${version.data.name} v${version.data.version}`
          : 'unknown',
      ok: version.isLoading ? null : !version.isError,
    },
  ];

  return (
    <section aria-labelledby="status-heading">
      <Card>
        <CardHeader>
          <CardTitle id="status-heading" className="text-base">
            System status
          </CardTitle>
          <CardDescription>Live connection to the Baitler backend.</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className="grid gap-3 sm:grid-cols-3">
            {rows.map((row) => (
              <div
                key={row.label}
                className="flex items-center justify-between rounded-md border border-border bg-muted/40 px-3 py-2"
              >
                <dt className="text-sm text-muted-foreground">{row.label}</dt>
                <dd className="flex items-center gap-1.5 text-sm font-medium">
                  <span
                    className={cn(
                      'h-2 w-2 rounded-full',
                      row.ok === null
                        ? 'bg-muted-foreground'
                        : row.ok
                          ? 'bg-success'
                          : 'bg-danger',
                    )}
                    aria-hidden="true"
                  />
                  {row.value}
                </dd>
              </div>
            ))}
          </dl>
          {health.isError && (
            <p className="mt-3 text-xs text-muted-foreground" role="status" aria-live="polite">
              Can&apos;t reach the API. Start the backend with{' '}
              <code className="rounded bg-muted px-1 py-0.5 font-mono">./scripts/dev.sh</code>.
            </p>
          )}
        </CardContent>
      </Card>
    </section>
  );
}
