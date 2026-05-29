import { Link } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';

import type { NavItem } from '@/config/navigation';
import { Badge } from '@/components/ui/badge';

/** Generic "coming in a later phase" page for not-yet-built feature routes. */
export function FeaturePlaceholder({ item }: { item: NavItem }) {
  const Icon = item.icon;
  return (
    <div className="mx-auto flex max-w-xl flex-col items-center gap-4 py-16 text-center">
      <span className="grid h-16 w-16 place-items-center rounded-2xl bg-accent text-accent-foreground">
        <Icon className="h-8 w-8" aria-hidden="true" />
      </span>
      <div className="flex items-center gap-2">
        <h1 className="text-2xl font-bold tracking-tight">{item.label}</h1>
        {item.phase !== null && <Badge variant="muted">Phase {item.phase}</Badge>}
      </div>
      <p className="text-muted-foreground">{item.description}</p>
      <p className="text-sm text-muted-foreground">
        This workspace arrives in a later phase of the build.
      </p>
      <Link
        to="/"
        className="inline-flex h-10 items-center gap-2 rounded-md border border-border px-4 text-sm font-medium transition-colors hover:bg-muted"
      >
        <ArrowLeft className="h-4 w-4" aria-hidden="true" />
        Back to dashboard
      </Link>
    </div>
  );
}
