import { apiStatusOf, useHealth } from '@/hooks/useSystemStatus';
import { Badge } from '@/components/ui/badge';
import { Spinner } from '@/components/ui/spinner';

/** Live indicator of backend reachability, polled via TanStack Query. */
export function ApiStatusBadge() {
  const health = useHealth();
  const status = apiStatusOf(health);

  if (status === 'loading') {
    return (
      <Badge variant="muted">
        <Spinner className="h-3 w-3" label="Checking API" />
        Checking API…
      </Badge>
    );
  }

  if (status === 'offline') {
    return (
      <Badge variant="danger" role="status" aria-live="polite">
        <span className="h-2 w-2 rounded-full bg-danger" aria-hidden="true" />
        API offline
      </Badge>
    );
  }

  return (
    <Badge variant="success" role="status" aria-live="polite">
      <span className="h-2 w-2 rounded-full bg-success" aria-hidden="true" />
      API connected
    </Badge>
  );
}
